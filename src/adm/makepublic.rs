use super::result::Error;
use crate::models::Photo;
use crate::schema::photo_tags::dsl as pt;
use crate::schema::photos::dsl as p;
use crate::schema::tags::dsl as t;
use crate::DbOpt;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::update;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
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
    pub async fn run(&self) -> Result<(), Error> {
        let mut db = self.db.connect().await?;
        match (
            self.list.as_ref().map(AsRef::as_ref),
            &self.tag,
            &self.image,
        ) {
            (Some("-"), None, None) => {
                let list = io::stdin();
                by_file_list(&mut db, list.lock()).await?;
                Ok(())
            }
            (Some(list), None, None) => {
                let list = BufReader::new(File::open(list)?);
                by_file_list(&mut db, list).await
            }
            (None, Some(tag), None) => {
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
                .execute(&mut db)
                .await?;
                println!("Made {n} images public.");
                Ok(())
            }
            (None, None, Some(image)) => one(&mut db, image).await,
            (None, None, None) => Err(Error::Other(
                "No images specified to make public".to_string(),
            )),
            _ => Err(Error::Other("Conflicting arguments".to_string())),
        }
    }
}

async fn one(db: &mut AsyncPgConnection, tpath: &str) -> Result<(), Error> {
    match update(p::photos.filter(p::path.eq(&tpath)))
        .set(p::is_public.eq(true))
        .get_result::<Photo>(db)
        .await
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

async fn by_file_list<In: BufRead + Sized>(
    db: &mut AsyncPgConnection,
    list: In,
) -> Result<(), Error> {
    for line in list.lines() {
        one(db, &line?).await?;
    }
    Ok(())
}
