use adm::result::Error;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::update;
use models::Photo;
use std::io::prelude::*;

pub fn one(db: &PgConnection, tpath: &str) -> Result<(), Error> {
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
            Err(Error::Other(format!("File {} is not known", tpath)))
        }
        Err(error) => Err(error.into()),
    }
}

pub fn by_file_list<In: BufRead + Sized>(
    db: &PgConnection,
    list: In,
) -> Result<(), Error> {
    for line in list.lines() {
        one(db, &line?)?;
    }
    Ok(())
}
