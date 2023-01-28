use super::result::Error;
use crate::models::Photo;
use crate::DbOpt;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::update;
use std::fs::File;
use std::io::prelude::*;
use std::io::{self, BufReader};

#[derive(clap::Parser)]
pub struct Makepublic {
    #[clap(flatten)]
    db: DbOpt,
    /// Image path to make public
    #[clap(group = "spec")]
    image: Option<String>,
    /// File listing image paths to make public
    #[clap(long, short, group = "spec")]
    list: Option<String>,
    /// Make all images with matching tag public.
    ///
    /// The tag is specified by its slug.
    #[clap(long, short, group = "spec")]
    tag: Option<String>,
}

impl Makepublic {
    pub fn run(&self) -> Result<(), Error> {
        let mut db = self.db.connect()?;
        match (
            self.list.as_ref().map(AsRef::as_ref),
            &self.tag,
            &self.image,
        ) {
            (Some("-"), None, None) => {
                let list = io::stdin();
                by_file_list(&mut db, list.lock())?;
                Ok(())
            }
            (Some(list), None, None) => {
                let list = BufReader::new(File::open(list)?);
                by_file_list(&mut db, list)
            }
            (None, Some(tag), None) => {
                use crate::schema::photo_tags::dsl as pt;
                use crate::schema::photos::dsl as p;
                use crate::schema::tags::dsl as t;
                let n = update(
                    p::photos.filter(
                        p::id.eq_any(
                            pt::photo_tags
                                .select(pt::photo_id)
                                .left_join(t::tags)
                                .filter(t::slug.eq(tag)),
                        ),
                    ),
                )
                .set(p::is_public.eq(true))
                .execute(&mut db)?;
                println!("Made {n} images public.");
                Ok(())
            }
            (None, None, Some(image)) => one(&mut db, image),
            (None, None, None) => Err(Error::Other(
                "No images specified to make public".to_string(),
            )),
            _ => Err(Error::Other("Conflicting arguments".to_string())),
        }
    }
}

pub fn one(db: &mut PgConnection, tpath: &str) -> Result<(), Error> {
    use crate::schema::photos::dsl::*;
    match update(photos.filter(path.eq(&tpath)))
        .set(is_public.eq(true))
        .get_result::<Photo>(db)
    {
        Ok(photo) => {
            println!("Made {tpath} public: {photo:?}");
            Ok(())
        }
        Err(DieselError::NotFound) => {
            Err(Error::Other(format!("File {tpath} is not known",)))
        }
        Err(error) => Err(error.into()),
    }
}

pub fn by_file_list<In: BufRead + Sized>(
    db: &mut PgConnection,
    list: In,
) -> Result<(), Error> {
    for line in list.lines() {
        one(db, &line?)?;
    }
    Ok(())
}
