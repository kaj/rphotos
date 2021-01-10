use super::result::Error;
use crate::models::{Photo, SizeTag};
use crate::photosdir::{get_scaled_jpeg, PhotosDir};
use crate::schema::photos::dsl::{date, is_public};
use crate::{CacheOpt, DbOpt, DirOpt};
use diesel::prelude::*;
use log::{debug, info};
use r2d2_memcache::memcache::Client;
use std::time::{Duration, Instant};
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct Args {
    #[structopt(flatten)]
    cache: CacheOpt,
    #[structopt(flatten)]
    db: DbOpt,
    #[structopt(flatten)]
    photos: DirOpt,

    /// Max time (in seconds) to work.
    #[structopt(long, short = "t", default_value = "10")]
    max_time: u64,
}

impl Args {
    /// Make sure all photos are stored in the cache.
    ///
    /// The work are intentionally handled sequentially, to not
    /// overwhelm the host while precaching.
    /// The images are handled in public first, new first order, to have
    /// the probably most requested images precached as soon as possible.
    pub async fn run(&self) -> Result<(), Error> {
        let max_time = Duration::from_secs(self.max_time);
        let timer = Instant::now();
        let cache = Client::connect(self.cache.memcached_url.as_ref())?;
        let size = SizeTag::Small;
        let (mut n, mut n_stored) = (0, 0);
        let photos = Photo::query(true)
            .order((is_public.desc(), date.desc().nulls_last()))
            .load::<Photo>(&self.db.connect()?)?;
        let no_expire = 0;
        let pd = PhotosDir::new(&self.photos.photos_dir);
        for photo in photos {
            n += 1;
            let key = &photo.cache_key(size);
            if cache.get::<Vec<u8>>(key)?.is_none() {
                let path = pd.get_raw_path(&photo);
                let size = size.px();
                let data = get_scaled_jpeg(path, photo.rotation, size)
                    .await
                    .map_err(|e| {
                    Error::Other(format!(
                        "Failed to scale #{} ({}): {:?}",
                        photo.id, photo.path, e,
                    ))
                })?;
                cache.set(key, &data[..], no_expire)?;
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
}
