use adm::result::Error;
use chrono::naive::NaiveDateTime;
use diesel::insert_into;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use models::{Camera, Modification, Photo};
use photosdir::PhotosDir;
use rexif::{ExifData, ExifEntry, ExifTag, TagValue};
use std::path::Path;

pub fn crawl(
    db: &PgConnection,
    photos: &PhotosDir,
    only_in: &Path,
) -> Result<(), Error> {
    Ok(photos.find_files(only_in, &|path, exif| match save_photo(
        db,
        path,
        exif,
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
    let photo = match Photo::create_or_set_basics(
        db,
        file_path,
        find_date(exif).ok(),
        find_rotation(exif)?,
        find_camera(db, exif)?,
    )? {
        Modification::Created(photo) => {
            info!("Created {:?}", photo);
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
    if let Some((lat, long)) = find_position(exif)? {
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
            use models::NewPosition;
            insert_into(positions)
                .values(&NewPosition {
                    photo_id: photo.id,
                    latitude: (lat * 1e6) as i32,
                    longitude: (long * 1e6) as i32,
                })
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
    if let (Some(maketag), Some(modeltag)) = (
        find_entry(exif, &ExifTag::Make),
        find_entry(exif, &ExifTag::Model),
    ) {
        if let (TagValue::Ascii(make), TagValue::Ascii(model)) =
            (maketag.clone().value, modeltag.clone().value)
        {
            let cam = Camera::get_or_create(db, &make, &model)?;
            return Ok(Some(cam));
        }
        // TODO else Err(...?)
    }
    Ok(None)
}

fn find_rotation(exif: &ExifData) -> Result<i16, Error> {
    if let Some(value) = find_entry(exif, &ExifTag::Orientation) {
        if let TagValue::U16(ref v) = value.value {
            let n = v[0];
            debug!("Raw orientation is {}", n);
            match n {
                1 | 0 => Ok(0),
                3 => Ok(180),
                6 => Ok(90),
                8 => Ok(270),
                x => Err(Error::UnknownOrientation(x)),
            }
        } else {
            Err(Error::Other(format!(
                "Exif of unexpectedType {:?}",
                value.value,
            )))
        }
    } else {
        info!("Orientation tag missing, default to 0 degrees");
        Ok(0)
    }
}

fn find_date(exif: &ExifData) -> Result<NaiveDateTime, Error> {
    find_entry(exif, &ExifTag::DateTimeOriginal)
        .or_else(|| find_entry(exif, &ExifTag::DateTime))
        .or_else(|| find_entry(exif, &ExifTag::DateTimeDigitized))
        .map(|value| {
            debug!("Found {:?}", value);
            if let TagValue::Ascii(ref str) = value.value {
                debug!(
                    "Try to parse {:?} (from {:?}) as datetime",
                    str, value.tag,
                );
                Ok(NaiveDateTime::parse_from_str(str, "%Y:%m:%d %T")?)
            } else {
                Err(Error::Other(format!(
                    "Exif of unexpectedType {:?}",
                    value.value,
                )))
            }
        })
        .unwrap_or_else(|| {
            Err(Error::Other(format!(
                "Exif tag missing: {:?}",
                ExifTag::DateTimeOriginal,
            )))
        })
}

fn find_position(exif: &ExifData) -> Result<Option<(f64, f64)>, Error> {
    if let Some(lat) = find_entry(exif, &ExifTag::GPSLatitude) {
        if let Some(long) = find_entry(exif, &ExifTag::GPSLongitude) {
            return Ok(Some((rat2float(&lat.value)?, rat2float(&long.value)?)));
        }
    }
    Ok(None)
}

fn rat2float(val: &TagValue) -> Result<f64, Error> {
    if let TagValue::URational(ref v) = *val {
        if v.len() == 3 {
            let (v0, v1, v2) = (v[0].value(), v[1].value(), v[2].value());
            return Ok(v0 + (v1 + v2 / 60.0) / 60.0);
        }
    }
    Err(Error::Other(format!("Bad lat/long value: {:?}", val)))
}

fn find_entry<'a>(exif: &'a ExifData, tag: &ExifTag) -> Option<&'a ExifEntry> {
    for entry in &exif.entries {
        if entry.tag == *tag {
            return Some(entry);
        }
    }
    None
}
