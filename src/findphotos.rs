#[macro_use] extern crate log;
extern crate chrono;
extern crate env_logger;
extern crate image;
extern crate rexif;
extern crate rustc_serialize;
extern crate rustorm;

use chrono::datetime::DateTime;
use chrono::format::ParseError;
use chrono::naive::datetime::NaiveDateTime;
use chrono::offset::TimeZone;
use chrono::offset::utc::UTC;
use rexif::{ExifData, ExifEntry, ExifTag, TagValue};
use rustorm::database::{Database, DbError};
use rustorm::pool::ManagedPool;
use rustorm::query::Query;
use rustorm::table::IsTable;
use std::path::Path;

mod env;
use env::{dburl, photos_dir};
mod photosdir;
use photosdir::PhotosDir;
mod models;
use models::{Photo, get_or_create};


fn main() {
    env_logger::init().unwrap();
    let pool = ManagedPool::init(&dburl(), 1).unwrap();
    let db = pool.connect().unwrap();
    let photos = PhotosDir::new(photos_dir());

    let only_in = Path::new("2016"); // TODO Get from command line!
    photos.find_files(only_in, &|path, exif| {
        match save_photo(db.as_ref(), path, &exif) {
            Ok(()) => debug!("Saved photo {}", path),
            Err(e) => warn!("Failed to save photo {}: {:?}", path, e),
        }
    }).unwrap();
}

fn save_photo(db: &Database, path: &str, exif: &ExifData) -> Result<(), FindPhotoError> {
    let date = &try!(find_date(&exif));
    let rotation = &try!(find_rotation(&exif));
    let photo: Photo = get_or_create(db, "path", &path.to_string(), &[
        ("date", date),
        ("rotation", rotation),
        ]);
    if *date != photo.date.unwrap() {
        panic!("Should update date for {} from {:?} to {:?}", path, photo.date, date);
    }
    if *rotation != photo.rotation {
        let mut q = Query::update();
        q.table(&Photo::table());
        q.filter_eq("id", &photo.id);
        q.set("rotation", rotation);
        try!(q.execute(db));
    }
    Ok(())
}

#[derive(Debug)]
enum FindPhotoError {
    DatabaseError(DbError),
    ExifOfUnexpectedType(TagValue),
    ExifTagMissing(ExifTag),
    TimeFormat(ParseError),
    UnknownOrientation(u16),
}
impl From<ParseError> for FindPhotoError {
    fn from(err: ParseError) -> FindPhotoError {
        FindPhotoError::TimeFormat(err)
    }
}
impl From<DbError> for FindPhotoError {
    fn from(err: DbError) -> FindPhotoError {
        FindPhotoError::DatabaseError(err)
    }
}

fn find_rotation(exif: &ExifData) -> Result<i16, FindPhotoError> {
    if let Some(ref value) = find_entry(exif, &ExifTag::Orientation) {
        if let TagValue::U16(ref v) = value.value {
            let n = v[0];
            info!("Raw orientation is {}", n);
            match n {
                1 => Ok(0),
                3 => Ok(180),
                6 => Ok(90),
                8 => Ok(270),
                x => Err(FindPhotoError::UnknownOrientation(x)),
            }
        } else {
            Err(FindPhotoError::ExifOfUnexpectedType(value.value.clone()))
        }
    } else {
        info!("Orientation tag missing, default to 0 degrees");
        Ok(0)
    }
}

fn find_date(exif: &ExifData) -> Result<DateTime<UTC>, FindPhotoError> {
    if let Some(ref value) = find_entry(exif, &ExifTag::DateTimeOriginal) {
        if let TagValue::Ascii(ref str) = value.value {
            let utc = UTC;
            debug!("Try to parse {:?} as datetime", str);
            Ok(utc.from_local_datetime(&try!(
                NaiveDateTime::parse_from_str(str, "%Y:%m:%d %T"))).latest().unwrap())
        } else {
            Err(FindPhotoError::ExifOfUnexpectedType(value.value.clone()))
        }
    } else {
        Err(FindPhotoError::ExifTagMissing(ExifTag::DateTimeOriginal))
    }
}

fn find_entry<'a>(exif: &'a ExifData, tag: &ExifTag) -> Option<&'a ExifEntry> {
    for entry in &exif.entries {
        if entry.tag == *tag {
            return Some(entry);
        }
    }
    None
}
