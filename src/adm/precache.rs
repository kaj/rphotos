use adm::result::Error;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use memcached::Client;
use memcached::proto::{Operation, ProtoType};
use models::Photo;
use photosdir::PhotosDir;
use schema::photos::dsl::{is_public, date};
use server::SizeTag;

pub fn precache(db: &PgConnection, pd: &PhotosDir) -> Result<(), Error> {
    let mut cache = Client::connect(&[("tcp://127.0.0.1:11211", 1)],
                                    ProtoType::Binary)?;
    let size = SizeTag::Small;
    let (mut n, mut n_stored) = (0, 0);
    let photos = Photo::query(true)
        .order((is_public.desc(), date.desc().nulls_last()))
        .load::<Photo>(db)?;
    for photo in photos {
        n = n + 1;
        let key = &photo.cache_key(&size);
        debug!("Cache: {:?} for {}.", key, photo.path);
        if cache.get(&key.as_bytes()).is_ok() {
            debug!("Cache: {} found", key);
        } else {
            let size = size.px();
            let data = pd.scale_image(&photo, size, size)
                .map_err(|e| {
                    Error::Other(format!("Failed to scale #{} ({}): {}",
                                         photo.id, photo.path, e))
                })?;
            cache.set(key.as_bytes(), &data, 0, 7 * 24 * 60 * 60)?;
            info!("Cache: stored {}", key);
            n_stored = n_stored + 1;
            if n_stored % 64 == 0 {
                info!("{} images of {} updated in cache ...", n_stored, n);
            }
        }
    }
    info!("{} images of {} updated in cache.", n_stored, n);
    Ok(())
}
