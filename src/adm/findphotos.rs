use super::result::Error;
use crate::models::{Camera, Modification, Photo};
use crate::myexif::ExifData;
use crate::photosdir::PhotosDir;
use crate::{DbOpt, DirOpt};
use diesel::insert_into;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use log::{debug, info, warn};
use std::path::Path;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct Findphotos {
    #[structopt(flatten)]
    db: DbOpt,
    #[structopt(flatten)]
    photos: DirOpt,

    /// Base directory to search in (relative to the image root).
    base: Vec<String>,
}

impl Findphotos {
    pub fn run(&self) -> Result<(), Error> {
        let pd = PhotosDir::new(&self.photos.photos_dir);
        let db = self.db.connect()?;
        if !self.base.is_empty() {
            for base in &self.base {
                crawl(&db, &pd, Path::new(base)).map_err(|e| {
                    Error::Other(format!("Failed to crawl {}: {}", base, e))
                })?;
            }
        } else {
            crawl(&db, &pd, Path::new("")).map_err(|e| {
                Error::Other(format!("Failed to crawl: {}", e))
            })?;
        }
        Ok(())
    }
}

fn crawl(
    db: &PgConnection,
    photos: &PhotosDir,
    only_in: &Path,
) -> Result<(), Error> {
    photos.find_files(
        only_in,
        &|path, exif| match save_photo(db, path, exif) {
            Ok(()) => debug!("Saved photo {}", path),
            Err(e) => warn!("Failed to save photo {}: {:?}", path, e),
        },
    )?;
    Ok(())
}

fn save_photo(
    db: &PgConnection,
    file_path: &str,
    exif: &ExifData,
) -> Result<(), Error> {
    let width = exif.width.ok_or(Error::MissingWidth)?;
    let height = exif.height.ok_or(Error::MissingHeight)?;
    let photo = match Photo::create_or_set_basics(
        db,
        file_path,
        width as i32,
        height as i32,
        exif.date(),
        exif.rotation()?,
        find_camera(db, exif)?,
    )? {
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
        {
            let lat = (lat * 1e6) as i32;
            let long = (long * 1e6) as i32;
            if clat != lat || clong != long {
                warn!(
                    "Photo #{}: {}: \
                     Exif position {}, {} differs from saved {}, {}",
                    photo.id, photo.path, clat, clong, lat, long,
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
                .expect("Insert image position");
        }
    }
    Ok(())
}

fn find_camera(
    db: &PgConnection,
    exif: &ExifData,
) -> Result<Option<Camera>, Error> {
    if let Some((make, model)) = exif.camera() {
        let cam = Camera::get_or_create(db, make, model)?;
        return Ok(Some(cam));
    }
    Ok(None)
}
