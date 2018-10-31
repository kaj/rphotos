use super::result::Error;
use crate::models::Photo;
use crate::photosdir::PhotosDir;
use crate::schema::photos::dsl::{date, is_public};
use crate::server::SizeTag;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use log::{debug, info};
use memcached::proto::{Operation, ProtoType};
use memcached::Client;
use std::time::{Duration, Instant};

/// Make sure all photos are stored in the cache.
///
/// The work are intentionally handled sequentially, to not
/// overwhelm the host while precaching.
/// The images are handled in public first, new first order, to have
/// the probably most requested images precached as soon as possible.
pub fn precache(
    db: &PgConnection,
    pd: &PhotosDir,
    max_secs: u64,
) -> Result<(), Error> {
    let max_time = Duration::from_secs(max_secs);
    let timer = Instant::now();
    let mut cache =
        Client::connect(&[("tcp://127.0.0.1:11211", 1)], ProtoType::Binary)?;
    let size = SizeTag::Small;
    let (mut n, mut n_stored) = (0, 0);
    let photos = Photo::query(true)
        .order((is_public.desc(), date.desc().nulls_last()))
        .load::<Photo>(db)?;
    let no_expire = 0;
    for photo in photos {
        n += 1;
        let key = &photo.cache_key(size);
        if cache.get(key.as_bytes()).is_err() {
            let size = size.px();
            let data = pd.scale_image(&photo, size, size).map_err(|e| {
                Error::Other(format!(
                    "Failed to scale #{} ({}): {}",
                    photo.id, photo.path, e,
                ))
            })?;
            cache.set(key.as_bytes(), &data, 0, no_expire)?;
            debug!("Cache: stored {} for {}", key, photo.path);
            n_stored += 1;
            if timer.elapsed() > max_time {
                break;
            }
            if n_stored % 64 == 0 {
                info!(
                    "Checked {} images in cache, added {}, in {:.1?}.",
                    n,
                    n_stored,
                    timer.elapsed()
                );
            }
        }
    }
    info!(
        "Checked {} images in cache, added {}, in {:.1?}.",
        n,
        n_stored,
        timer.elapsed()
    );
    Ok(())
}
