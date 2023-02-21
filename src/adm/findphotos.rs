use super::result::Error;
use crate::models::{Camera, Modification, Photo};
use crate::myexif::ExifData;
use crate::photosdir::{load_meta, PhotosDir};
use crate::{DbOpt, DirOpt};
use diesel::insert_into;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use std::path::Path;
use tracing::{debug, info, warn};

#[derive(clap::Parser)]
pub struct Findphotos {
    #[clap(flatten)]
    db: DbOpt,
    #[clap(flatten)]
    photos: DirOpt,

    /// Base directory to search in (relative to the image root).
    base: Vec<String>,
}

impl Findphotos {
    pub async fn run(&self) -> Result<(), Error> {
        let pd = PhotosDir::new(&self.photos.photos_dir);
        let mut db = self.db.connect().await?;
        if !self.base.is_empty() {
            for base in &self.base {
                crawl(&mut db, &pd, Path::new(base)).await.map_err(|e| {
                    Error::Other(format!("Failed to crawl {base}: {e}"))
                })?;
            }
        } else {
            crawl(&mut db, &pd, Path::new(""))
                .await
                .map_err(|e| Error::Other(format!("Failed to crawl: {e}")))?;
        }
        Ok(())
    }
}

async fn crawl(
    db: &mut AsyncPgConnection,
    photos: &PhotosDir,
    only_in: &Path,
) -> Result<(), Error> {
    use futures_lite::stream::StreamExt;
    let mut entries = photos.walk_dir(only_in);
    loop {
        match entries.next().await {
            None => break,
            Some(Err(e)) => return Err(e.into()),
            Some(Ok(entry)) => {
                if entry.file_type().await?.is_file() {
                    let path = entry.path();
                    if let Some(exif) = load_meta(&path) {
                        let sp = photos.subpath(&path)?;
                        save_photo(db, sp, &exif).await?;
                    } else {
                        debug!("Not an image: {path:?}");
                    }
                }
            }
        }
    }
    Ok(())
}

async fn save_photo(
    db: &mut AsyncPgConnection,
    file_path: &str,
    exif: &ExifData,
) -> Result<(), Error> {
    let width = exif.width.ok_or(Error::MissingWidth)?;
    let height = exif.height.ok_or(Error::MissingHeight)?;
    let rot = exif.rotation()?;
    let cam = find_camera(db, exif).await?;
    let photo = match Photo::create_or_set_basics(
        db,
        file_path,
        width as i32,
        height as i32,
        exif.date(),
        rot,
        cam,
    )
    .await?
    {
        Modification::Created(photo) => {
            info!("Created #{}, {}", photo.id, photo.path);
            photo
        }
        Modification::Updated(photo) => {
            info!("Modified {:?}", photo);
            photo
        }
        Modification::Unchanged(photo) => {
            debug!("No change for {:?}", photo);
            photo
        }
    };
    if let Some((lat, long)) = exif.position() {
        debug!("Position for {} is {} {}", file_path, lat, long);
        use crate::schema::positions::dsl::*;
        if let Ok((clat, clong)) = positions
            .filter(photo_id.eq(photo.id))
            .select((latitude, longitude))
            .first::<(i32, i32)>(db)
            .await
        {
            let lat = (lat * 1e6) as i32;
            let long = (long * 1e6) as i32;
            if (clat - lat).abs() > 1000 || (clong - long).abs() > 1000 {
                warn!(
                    "Photo #{}: {}: \
                     Exif position {}, {} differs from saved {}, {}",
                    photo.id, photo.path, lat, long, clat, clong,
                );
            }
        } else {
            info!("Position for {} is {} {}", file_path, lat, long);
            insert_into(positions)
                .values((
                    photo_id.eq(photo.id),
                    latitude.eq((lat * 1e6) as i32),
                    longitude.eq((long * 1e6) as i32),
                ))
                .execute(db)
                .await
                .expect("Insert image position");
        }
    }
    Ok(())
}

async fn find_camera(
    db: &mut AsyncPgConnection,
    exif: &ExifData,
) -> Result<Option<Camera>, Error> {
    if let Some((make, model)) = exif.camera() {
        let cam = Camera::get_or_create(db, make, model).await?;
        return Ok(Some(cam));
    }
    Ok(None)
}
