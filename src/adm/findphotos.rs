use adm::result::Error;
use chrono::naive::datetime::NaiveDateTime;
use diesel::insert;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use photosdir::PhotosDir;
use rexif::{ExifData, ExifEntry, ExifTag, TagValue};
use rphotos::models::{Modification, Photo, Camera};
use std::path::Path;

pub fn crawl(db: &PgConnection, photos: &PhotosDir, only_in: &Path)
             -> Result<(), Error> {
    try!(photos.find_files(only_in,
                           &|path, exif| {
        match save_photo(&db, path, &exif) {
            Ok(()) => debug!("Saved photo {}", path),
            Err(e) => warn!("Failed to save photo {}: {:?}", path, e),
        }
    }));
    Ok(())
}

fn save_photo(db: &PgConnection,
              file_path: &str,
              exif: &ExifData)
              -> Result<(), Error> {
    let photo =
        match try!(Photo::create_or_set_basics(db, file_path,
                                               Some(try!(find_date(&exif))),
                                               try!(find_rotation(&exif)),
                                               try!(find_camera(db, exif)))) {
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
    if let Some((lat, long)) = try!(find_position(&exif)) {
        debug!("Position for {} is {} {}", file_path, lat, long);
        use rphotos::schema::positions::dsl::*;
        if let Ok((pos, clat, clong)) =
            positions.filter(photo_id.eq(photo.id))
                .select((id, latitude, longitude))
                .first::<(i32, i32, i32)>(db) {
            if (clat != (lat * 1e6) as i32) || (clong != (long * 1e6) as i32) {
                panic!("TODO Should update position #{} from {} {} to {} {}",
                       pos, clat, clong, lat, long)
            }
        } else {
            info!("Position for {} is {} {}", file_path, lat, long);
            use rphotos::models::NewPosition;
            insert(&NewPosition {
                    photo_id: photo.id,
                    latitude: (lat * 1e6) as i32,
                    longitude: (long * 1e6) as i32,
                })
                .into(positions)
                .execute(db)
                .expect("Insert image position");
        }
    }
    Ok(())
}

fn find_camera(db: &PgConnection,
               exif: &ExifData)
               -> Result<Option<Camera>, Error> {
    if let (Some(maketag), Some(modeltag)) =
        (find_entry(exif, &ExifTag::Make), find_entry(exif, &ExifTag::Model)) {
        if let (TagValue::Ascii(make), TagValue::Ascii(model)) =
            (maketag.clone().value, modeltag.clone().value) {
            let cam = try!(Camera::get_or_create(db, &make, &model));
            return Ok(Some(cam));
        }
        // TODO else Err(...?)
    }
    Ok(None)
}

fn find_rotation(exif: &ExifData) -> Result<i16, Error> {
    if let Some(ref value) = find_entry(exif, &ExifTag::Orientation) {
        if let TagValue::U16(ref v) = value.value {
            let n = v[0];
            debug!("Raw orientation is {}", n);
            match n {
                1 => Ok(0),
                3 => Ok(180),
                6 => Ok(90),
                8 => Ok(270),
                x => Err(Error::UnknownOrientation(x)),
            }
        } else {
            Err(Error::Other(format!("Exif of unexpectedType {:?}",
                                     value.value)))
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
                debug!("Try to parse {:?} (from {:?}) as datetime",
                       str,
                       value.tag);
                Ok(try!(NaiveDateTime::parse_from_str(str, "%Y:%m:%d %T")))
            } else {
                Err(Error::Other(format!("Exif of unexpectedType {:?}",
                                         value.value)))
            }
        })
        .unwrap_or_else(|| {
            Err(Error::Other(format!("Exif tag missing: {:?}",
                                     ExifTag::DateTimeOriginal)))
        })
}

fn find_position(exif: &ExifData) -> Result<Option<(f64, f64)>, Error> {
    if let Some(lat) = find_entry(exif, &ExifTag::GPSLatitude) {
        if let Some(long) = find_entry(exif, &ExifTag::GPSLongitude) {
            return Ok(Some((try!(rat2float(&lat.value)),
                            try!(rat2float(&long.value)))));
        }
    }
    Ok(None)
}

fn rat2float(val: &TagValue) -> Result<f64, Error> {
    if let TagValue::URational(ref v) = *val {
        if v.len() == 3 {
            return Ok(v[0].value() +
                      (v[1].value() + v[2].value() / 60.0) / 60.0);
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
