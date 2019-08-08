use super::Args;
use crate::fetch_places::OverpassOpt;
use crate::photosdir::PhotosDir;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use log::{debug, warn};
use medallion::{Header, Payload, Token};
use r2d2_memcache::r2d2::Error;
use r2d2_memcache::MemcacheConnectionManager;
use std::sync::Arc;
use std::time::Duration;
use warp::filters::{cookie, BoxedFilter};
use warp::path::{self, FullPath};
use warp::{self, Filter};

type PgPool = Pool<ConnectionManager<PgConnection>>;
type PooledPg = PooledConnection<ConnectionManager<PgConnection>>;
type MemcachePool = Pool<MemcacheConnectionManager>;
type PooledMemcache = PooledConnection<MemcacheConnectionManager>;

pub fn create_session_filter(args: &Args) -> BoxedFilter<(Context,)> {
    let global = Arc::new(GlobalContext::new(args));
    warp::any()
        .and(path::full())
        .and(cookie::optional("EXAUTH"))
        .map(move |path, key: Option<String>| {
            let global = global.clone();
            let user = key.and_then(|k| {
                global
                    .verify_key(&k)
                    .map_err(|e| warn!("Auth failed: {}", e))
                    .ok()
            });
            Context { global, path, user }
        })
        .boxed()
}

// Does _not_ derive debug, copy or clone, since it contains the jwt
// secret and some connection pools.
struct GlobalContext {
    db_pool: PgPool,
    photosdir: PhotosDir,
    memcache_pool: MemcachePool,
    jwt_secret: String,
    overpass: OverpassOpt,
}

impl GlobalContext {
    fn new(args: &Args) -> Self {
        let db_manager =
            ConnectionManager::<PgConnection>::new(&args.db.db_url);
        let mc_manager =
            MemcacheConnectionManager::new(args.cache.memcached_url.as_ref());
        GlobalContext {
            db_pool: Pool::builder()
                .connection_timeout(Duration::from_secs(1))
                .build(db_manager)
                .expect("Posgresql pool"),
            photosdir: PhotosDir::new(&args.photos.photos_dir),
            memcache_pool: Pool::builder()
                .connection_timeout(Duration::from_secs(1))
                .build(mc_manager)
                .expect("Memcache pool"),
            jwt_secret: args.jwt_key.clone(),
            overpass: args.overpass.clone(),
        }
    }

    fn verify_key(&self, jwtstr: &str) -> Result<String, String> {
        let token = Token::<Header, ()>::parse(&jwtstr)
            .map_err(|e| format!("Bad jwt token: {:?}", e))?;

        if token.verify(self.jwt_secret.as_ref()).unwrap_or(false) {
            let claims = token.payload;
            debug!("Verified token for: {:?}", claims);
            let now = current_numeric_date();
            if let Some(nbf) = claims.nbf {
                if now < nbf {
                    return Err(format!(
                        "Not-yet valid token, {} < {}",
                        now, nbf,
                    ));
                }
            }
            if let Some(exp) = claims.exp {
                if now > exp {
                    return Err(format!(
                        "Got an expired token: {} > {}",
                        now, exp,
                    ));
                }
            }
            if let Some(user) = claims.sub {
                return Ok(user);
            } else {
                return Err("User missing in claims".to_string());
            }
        } else {
            Err(format!("Invalid token {:?}", token))
        }
    }
    fn cache(&self) -> Result<PooledMemcache, Error> {
        Ok(self.memcache_pool.get()?)
    }
}

/// The request context, providing database, memcache and authorized user.
pub struct Context {
    global: Arc<GlobalContext>,
    path: FullPath,
    user: Option<String>,
}

impl Context {
    pub fn db(&self) -> Result<PooledPg, String> {
        self.global
            .db_pool
            .get()
            .map_err(|e| format!("Failed to get db {}", e))
    }
    pub fn authorized_user(&self) -> Option<&str> {
        self.user.as_ref().map(AsRef::as_ref)
    }
    pub fn is_authorized(&self) -> bool {
        self.user.is_some()
    }
    pub fn path_without_query(&self) -> &str {
        self.path.as_str()
    }
    pub fn cached_or<F, E>(
        &self,
        key: &str,
        calculate: F,
    ) -> Result<Vec<u8>, E>
    where
        F: FnOnce() -> Result<Vec<u8>, E>,
    {
        match self.global.cache() {
            Ok(mut client) => {
                match client.get(key) {
                    Ok(Some(data)) => {
                        debug!("Cache: {} found", key);
                        return Ok(data);
                    }
                    Ok(None) => {
                        debug!("Cache: {} not found", key);
                    }
                    Err(err) => {
                        warn!("Cache: get {} failed: {:?}", key, err);
                    }
                }
                let data = calculate()?;
                match client.set(key, &data[..], 7 * 24 * 60 * 60) {
                    Ok(()) => debug!("Cache: stored {}", key),
                    Err(err) => warn!("Cache: Error storing {}: {}", key, err),
                }
                Ok(data)
            }
            Err(err) => {
                warn!("Error connecting to memcache: {}", err);
                calculate()
            }
        }
    }
    pub fn clear_cache(&self, key: &str) {
        if let Ok(mut client) = self.global.cache() {
            match client.delete(key) {
                Ok(flag) => debug!("Cache: deleted {}: {:?}", key, flag),
                Err(e) => warn!("Cache: Failed to delete {}: {}", key, e),
            }
        }
    }
    pub fn photos(&self) -> &PhotosDir {
        &self.global.photosdir
    }
    pub fn overpass(&self) -> &OverpassOpt {
        &self.global.overpass
    }

    pub fn make_token(&self, user: &str) -> Option<String> {
        let header: Header = Default::default();
        let now = current_numeric_date();
        let expiration_time = Duration::from_secs(14 * 24 * 60 * 60);
        let claims = Payload::<()> {
            iss: None, // TODO?
            sub: Some(user.into()),
            exp: Some(now + expiration_time.as_secs()),
            nbf: Some(now),
            ..Default::default()
        };
        let token = Token::new(header, claims);
        token.sign(self.global.jwt_secret.as_ref()).ok()
    }
}

/// Get the current value for jwt NumericDate.
///
/// Defined in RFC 7519 section 2 to be equivalent to POSIX.1 "Seconds
/// Since the Epoch".  The RFC allows a NumericDate to be non-integer
/// (for sub-second resolution), but the jwt crate uses u64.
fn current_numeric_date() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
