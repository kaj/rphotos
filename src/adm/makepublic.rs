use adm::result::Error;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::update;
use photosdir::PhotosDir;
use rphotos::models::{Modification, Photo};
use std::io::prelude::*;

pub fn one(db: &PgConnection,
           photodir: &PhotosDir,
           tpath: &str)
           -> Result<(), Error> {
    use rphotos::schema::photos::dsl::*;
    match update(photos.filter(path.eq(&tpath)))
        .set(is_public.eq(true))
        .get_result::<Photo>(db) {
        Ok(photo) => {
            println!("Made {} public: {:?}", tpath, photo);
            Ok(())
        }
        Err(DieselError::NotFound) => {
            if !photodir.has_file(&tpath) {
                return Err(Error::Other(format!("File {} does not exist",
                                                tpath)));
            }
            let photo = try!(register_photo(db, &tpath));
            println!("New photo {:?} is public.", photo);
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

pub fn by_file_list<In: BufRead + Sized>(db: &PgConnection,
                                         photodir: &PhotosDir,
                                         list: In)
                                         -> Result<(), Error> {
    for line in list.lines() {
        try!(one(db, photodir, &try!(line)));
    }
    Ok(())
}

fn register_photo(db: &PgConnection,
                  tpath: &str)
                  -> Result<Photo, DieselError> {
    use rphotos::schema::photos::dsl::{photos, is_public};
    let photo =
        match try!(Photo::create_or_set_basics(&db, &tpath, None, 0, None)) {
            Modification::Created(photo) => photo,
            Modification::Updated(photo) => photo,
            Modification::Unchanged(photo) => photo,
        };
    update(photos.find(photo.id))
        .set(is_public.eq(true))
        .get_result::<Photo>(db)
}
