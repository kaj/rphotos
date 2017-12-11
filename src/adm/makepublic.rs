use adm::result::Error;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::update;
use models::{Modification, Photo};
use photosdir::PhotosDir;
use std::io::prelude::*;

pub fn one(
    db: &PgConnection,
    photodir: &PhotosDir,
    tpath: &str,
) -> Result<(), Error> {
    use schema::photos::dsl::*;
    match update(photos.filter(path.eq(&tpath)))
        .set(is_public.eq(true))
        .get_result::<Photo>(db)
    {
        Ok(photo) => {
            println!("Made {} public: {:?}", tpath, photo);
            Ok(())
        }
        Err(DieselError::NotFound) => {
            if !photodir.has_file(&tpath) {
                return Err(Error::Other(format!(
                    "File {} does not exist",
                    tpath,
                )));
            }
            let photo = register_photo(db, tpath)?;
            println!("New photo {:?} is public.", photo);
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

pub fn by_file_list<In: BufRead + Sized>(
    db: &PgConnection,
    photodir: &PhotosDir,
    list: In,
) -> Result<(), Error> {
    for line in list.lines() {
        one(db, photodir, &line?)?;
    }
    Ok(())
}

fn register_photo(
    db: &PgConnection,
    tpath: &str,
) -> Result<Photo, DieselError> {
    use schema::photos::dsl::{is_public, photos};
    let photo = match Photo::create_or_set_basics(db, tpath, None, 0, None)? {
        Modification::Created(photo)
        | Modification::Updated(photo)
        | Modification::Unchanged(photo) => photo,
    };
    update(photos.find(photo.id))
        .set(is_public.eq(true))
        .get_result::<Photo>(db)
}
