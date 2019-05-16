use crate::photosdir::PhotosDir;
use crypto::sha2::Sha256;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use jwt::{Claims, Header, Registered, Token};
use log::{debug, error, warn};
use r2d2_memcache::r2d2::Error;
use r2d2_memcache::MemcacheConnectionManager;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use warp::filters::{cookie, BoxedFilter};
use warp::path::{self, FullPath};
use warp::reject::custom;
use warp::{self, Filter};

type PgPool = Pool<ConnectionManager<PgConnection>>;
type PooledPg = PooledConnection<ConnectionManager<PgConnection>>;
type MemcachePool = Pool<MemcacheConnectionManager>;
type PooledMemcache = PooledConnection<MemcacheConnectionManager>;

pub fn create_session_filter(
    db_url: &str,
    memcache_server: &str,
    photos_dir: &Path,
    jwt_secret: &str,
) -> BoxedFilter<(Context,)> {
    let global = Arc::new(GlobalContext::new(
        db_url,
        memcache_server,
        photos_dir,
        jwt_secret,
    ));
    warp::any()
        .and(path::full())
        .and(cookie::optional("EXAUTH"))
        .and_then(move |path, key: Option<String>| {
            let user = key.and_then(|k| {
                global
                    .verify_key(&k)
                    .map_err(|e| warn!("Auth failed: {}", e))
                    .ok()
            });
            Context::new(global.clone(), path, user).map_err(|e| {
                error!("Failed to initialize session: {}", e);
                custom(e)
            })
        })
        .boxed()
}

struct GlobalContext {
    db_pool: PgPool,
    photosdir: PhotosDir,
    memcache_pool: MemcachePool,
    jwt_secret: String,
}

impl GlobalContext {
    fn new(
        db_url: &str,
        memcache_server: &str,
        photos_dir: &Path,
        jwt_secret: &str,
    ) -> Self {
        let db_manager = ConnectionManager::<PgConnection>::new(db_url);
        let mc_manager = MemcacheConnectionManager::new(memcache_server);
        GlobalContext {
            db_pool: Pool::builder()
                .connection_timeout(Duration::from_secs(1))
                .build(db_manager)
                .expect("Posgresql pool"),
            photosdir: PhotosDir::new(photos_dir),
            memcache_pool: Pool::builder()
                .connection_timeout(Duration::from_secs(1))
                .build(mc_manager)
                .expect("Memcache pool"),
            jwt_secret: jwt_secret.into(),
        }
    }

    fn verify_key(&self, jwtstr: &str) -> Result<String, String> {
        let token = Token::<Header, Claims>::parse(&jwtstr)
            .map_err(|e| format!("Bad jwt token: {:?}", e))?;

        if token.verify(self.jwt_secret.as_ref(), Sha256::new()) {
            let claims = token.claims;
            debug!("Verified token for: {:?}", claims);
            let now = current_numeric_date();
            if let Some(nbf) = claims.reg.nbf {
                if now < nbf {
                    return Err(format!(
                        "Not-yet valid token, {} < {}",
                        now, nbf,
                    ));
                }
            }
            if let Some(exp) = claims.reg.exp {
                if now > exp {
                    return Err(format!(
                        "Got an expired token: {} > {}",
                        now, exp,
                    ));
                }
            }
            if let Some(user) = claims.reg.sub {
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
    db: PooledPg,
    path: FullPath,
    user: Option<String>,
}

impl Context {
    fn new(
        global: Arc<GlobalContext>,
        path: FullPath,
        user: Option<String>,
    ) -> Result<Self, String> {
        let db = global
            .db_pool
            .get()
            .map_err(|e| format!("Failed to get db {}", e))?;
        Ok(Context {
            global,
            db,
            path,
            user,
        })
    }
    pub fn db(&self) -> &PgConnection {
        &self.db
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

    pub fn make_token(&self, user: &str) -> Option<String> {
        let header: Header = Default::default();
        let now = current_numeric_date();
        let expiration_time = Duration::from_secs(14 * 24 * 60 * 60);
        let claims = Claims {
            reg: Registered {
                iss: None, // TODO?
                sub: Some(user.into()),
                exp: Some(now + expiration_time.as_secs()),
                nbf: Some(now),
                ..Default::default()
            },
            private: BTreeMap::new(),
        };
        let token = Token::new(header, claims);
        token
            .signed(self.global.jwt_secret.as_ref(), Sha256::new())
            .ok()
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
