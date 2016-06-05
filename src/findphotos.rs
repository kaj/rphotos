#[macro_use]
extern crate log;
extern crate chrono;
extern crate env_logger;
extern crate image;
extern crate rexif;
extern crate rustc_serialize;
extern crate dotenv;
extern crate diesel;
extern crate rphotos;

use chrono::format::ParseError;
use chrono::naive::datetime::NaiveDateTime;
use rexif::{ExifData, ExifEntry, ExifTag, TagValue};
use std::path::Path;
use dotenv::dotenv;
use diesel::pg::PgConnection;
use self::diesel::prelude::*;
use rphotos::models::{Modification, Photo};

mod env;
use env::{dburl, photos_dir};
mod photosdir;
use photosdir::PhotosDir;


fn main() {
    dotenv().ok();
    env_logger::init().unwrap();
    let db = PgConnection::establish(&dburl())
        .expect("Error connecting to database");
    let photos = PhotosDir::new(photos_dir());

    let args = std::env::args().skip(1);
    if args.len() > 0 {
        for a in args {
            do_find(&db, &photos, Path::new(&a));
        }
    } else {
        println!("No args");
    }
}

fn do_find(db: &PgConnection, photos: &PhotosDir, only_in: &Path) {
    photos.find_files(only_in,
                      &|path, exif| {
                          match save_photo(&db, path, &exif) {
                              Ok(()) => debug!("Saved photo {}", path),
                              Err(e) => {
                                  warn!("Failed to save photo {}: {:?}",
                                        path,
                                        e)
                              }
                          }
                      })
        .unwrap();
}

fn save_photo(db: &PgConnection,
              file_path: &str,
              exif: &ExifData)
              -> Result<(), FindPhotoError> {
    match try!(Photo::create_or_set_basics(db, file_path,
                                           Some(try!(find_date(&exif))),
                                           try!(find_rotation(&exif)))) {
        Modification::Created(photo) => info!("Created {:?}", photo),
        Modification::Updated(photo) => info!("Modified {:?}", photo),
        Modification::Unchanged(photo) => debug!("No change for {:?}", photo),
    };
    Ok(())
}

#[derive(Debug)]
enum FindPhotoError {
    DatabaseError(diesel::result::Error),
    ExifOfUnexpectedType(TagValue),
    ExifTagMissing(ExifTag),
    TimeFormat(ParseError),
    UnknownOrientation(u16),
}
impl From<diesel::result::Error> for FindPhotoError {
    fn from(err: diesel::result::Error) -> FindPhotoError {
        FindPhotoError::DatabaseError(err)
    }
}
impl From<ParseError> for FindPhotoError {
    fn from(err: ParseError) -> FindPhotoError {
        FindPhotoError::TimeFormat(err)
    }
}

fn find_rotation(exif: &ExifData) -> Result<i16, FindPhotoError> {
    if let Some(ref value) = find_entry(exif, &ExifTag::Orientation) {
        if let TagValue::U16(ref v) = value.value {
            let n = v[0];
            debug!("Raw orientation is {}", n);
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

fn find_date(exif: &ExifData) -> Result<NaiveDateTime, FindPhotoError> {
    if let Some(ref value) = find_entry(exif, &ExifTag::DateTimeOriginal) {
        if let TagValue::Ascii(ref str) = value.value {
            debug!("Try to parse {:?} as datetime", str);
            Ok(try!(NaiveDateTime::parse_from_str(str, "%Y:%m:%d %T")))
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
