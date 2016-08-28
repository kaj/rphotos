#[macro_use]
extern crate log;
extern crate env_logger;
extern crate dotenv;
extern crate diesel;
extern crate rphotos;
extern crate image;
extern crate rexif;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::result::Error;
use dotenv::dotenv;
use rphotos::models::{Modification, Photo};
use std::io::prelude::*;
use std::io;

mod env;
use env::{dburl, photos_dir};
mod photosdir;
use photosdir::PhotosDir;

fn main() {
    dotenv().ok();
    env_logger::init().unwrap();
    let photodir = PhotosDir::new(photos_dir());
    let db = PgConnection::establish(&dburl())
                 .expect("Error connecting to database");

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(line) => {
                use rphotos::schema::photos::dsl::*;
                match diesel::update(photos.filter(path.eq(&line)))
                    .set(is_public.eq(true))
                    .get_result::<Photo>(&db) {
                        Ok(photo) =>
                            info!("Made {} public: {:?}", line, photo),
                        Err(Error::NotFound) => {
                            if !photodir.has_file(&line) {
                                panic!("File {} does not exist", line);
                            }
                            let photo = register_photo(&db, &line)
                                .expect("Register photo");
                            info!("New photo {:?} is public.", photo);
                        }
                        Err(error) =>
                            panic!("Problem with {}: {:?}", line, error),
                    }
            }
            Err(err) => {
                panic!("Failed to read a line: {:?}", err);
            }
        }
    }
}

fn register_photo(db: &PgConnection, tpath: &str) -> Result<Photo, DieselError> {
    debug!("Should add {} to database", tpath);
    use rphotos::schema::photos::dsl::{photos, is_public};
    let photo =
        match try!(Photo::create_or_set_basics(&db, &tpath, None, 0, None)) {
            Modification::Created(photo) => photo,
            Modification::Updated(photo) => photo,
            Modification::Unchanged(photo) => photo
        };
    diesel::update(photos.find(photo.id))
        .set(is_public.eq(true))
        .get_result::<Photo>(db)
}
