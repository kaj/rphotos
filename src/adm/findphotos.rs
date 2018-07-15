use adm::result::Error;
use diesel::insert_into;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use models::{Camera, Modification, Photo};
use myexif::ExifData;
use photosdir::PhotosDir;
use std::path::Path;

pub fn crawl(
    db: &PgConnection,
    photos: &PhotosDir,
    only_in: &Path,
) -> Result<(), Error> {
    Ok(photos.find_files(only_in, &|path, exif| match save_photo(
        db, path, exif,
    ) {
        Ok(()) => debug!("Saved photo {}", path),
        Err(e) => warn!("Failed to save photo {}: {:?}", path, e),
    })?)
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
        use schema::positions::dsl::*;
        if let Ok((pos, clat, clong)) = positions
            .filter(photo_id.eq(photo.id))
            .select((id, latitude, longitude))
            .first::<(i32, i32, i32)>(db)
        {
            if (clat != (lat * 1e6) as i32) || (clong != (long * 1e6) as i32) {
                panic!(
                    "TODO Should update position #{} from {} {} to {} {}",
                    pos, clat, clong, lat, long,
                )
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
        let cam = Camera::get_or_create(db, &make, &model)?;
        return Ok(Some(cam));
    }
    Ok(None)
}
