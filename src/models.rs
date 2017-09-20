use chrono::naive::NaiveDateTime;
use diesel::pg::PgConnection;
use diesel::result::Error as DieselError;
use server::SizeTag;

#[derive(Debug, Clone, Queryable)]
pub struct Photo {
    pub id: i32,
    pub path: String,
    pub date: Option<NaiveDateTime>,
    pub grade: Option<i16>,
    pub rotation: i16,
    pub is_public: bool,
    pub camera_id: Option<i32>,
    pub attribution_id: Option<i32>,
}

use schema::photos;
#[derive(Debug, Clone, Insertable)]
#[table_name="photos"]
pub struct NewPhoto<'a> {
    pub path: &'a str,
    pub date: Option<NaiveDateTime>,
    pub rotation: i16,
    pub camera_id: Option<i32>,
}

#[derive(Debug)]
pub enum Modification<T> {
    Created(T),
    Updated(T),
    Unchanged(T),
}

use diesel::pg::Pg;
impl Photo {
    #[allow(dead_code)]
    pub fn is_public(&self) -> bool {
        self.is_public
    }

    pub fn cache_key(&self, size: &SizeTag) -> String {
        format!("rp{}{:?}", self.id, size)
    }

    #[allow(dead_code)]
    pub fn query<'a>(auth: bool) -> photos::BoxedQuery<'a, Pg> {
        use super::schema::photos::dsl::{is_public, photos, path};
        use diesel::prelude::*;
        let result = photos
            .filter(path.not_like("%.CR2"))
            .filter(path.not_like("%.dng"))
            .into_boxed();
        if !auth {
            result.filter(is_public)
        } else {
            result
        }
    }

    pub fn update_by_path(db: &PgConnection,
                          file_path: &str,
                          exifdate: Option<NaiveDateTime>,
                          exifrotation: i16,
                          camera: &Option<Camera>)
                          -> Result<Option<Modification<Photo>>, DieselError> {
        use diesel;
        use diesel::prelude::*;
        use schema::photos::dsl::*;
        if let Some(mut pic) =
               photos.filter(path.eq(&file_path.to_string()))
                    .first::<Photo>(db)
                    .optional()?
        {
            let mut change = false;
            // TODO Merge updates to one update statement!
            if exifdate.is_some() && exifdate != pic.date {
                change = true;
                pic = diesel::update(photos.find(pic.id))
                               .set(date.eq(exifdate))
                               .get_result::<Photo>(db)?;
            }
            if exifrotation != pic.rotation {
                change = true;
                pic = diesel::update(photos.find(pic.id))
                               .set(rotation.eq(exifrotation))
                               .get_result::<Photo>(db)?;
            }
            if let &Some(ref camera) = camera {
                if pic.camera_id != Some(camera.id) {
                    change = true;
                    pic = diesel::update(photos.find(pic.id))
                               .set(camera_id.eq(camera.id))
                               .get_result::<Photo>(db)?;
                }
            }
            Ok(Some(if change {
                Modification::Updated(pic)
            } else {
                Modification::Unchanged(pic)
            }))
        } else {
            Ok(None)
        }
    }

    pub fn create_or_set_basics(db: &PgConnection,
                                file_path: &str,
                                exifdate: Option<NaiveDateTime>,
                                exifrotation: i16,
                                camera: Option<Camera>)
                                -> Result<Modification<Photo>, DieselError> {
        use diesel;
        use diesel::prelude::*;
        use schema::photos::dsl::*;
        if let Some(result) = Self::update_by_path(db, file_path, exifdate, exifrotation, &camera)? {
            Ok(result)
        } else {
            let pic = NewPhoto {
                path: &file_path,
                date: exifdate,
                rotation: exifrotation,
                camera_id: camera.map(|c| c.id),
            };
            let pic = diesel::insert(&pic)
                           .into(photos)
                           .get_result::<Photo>(db)?;
            Ok(Modification::Created(pic))
        }
    }
}

#[derive(Debug, Clone, Queryable)]
pub struct Tag {
    pub id: i32,
    pub slug: String,
    pub tag_name: String,
}


#[derive(Debug, Clone, Queryable)]
pub struct PhotoTag {
    pub id: i32,
    pub photo_id: i32,
    pub tag_id: i32,
}

use super::schema::tags;
#[derive(Debug, Clone, Insertable)]
#[table_name="tags"]
pub struct NewTag<'a> {
    pub tag_name: &'a str,
    pub slug: &'a str,
}

use super::schema::photo_tags;
#[derive(Debug, Clone, Insertable)]
#[table_name="photo_tags"]
pub struct NewPhotoTag {
    pub photo_id: i32,
    pub tag_id: i32,
}


#[derive(Debug, Clone, Queryable)]
pub struct Person {
    pub id: i32,
    pub slug: String,
    pub person_name: String,
}

#[derive(Debug, Clone, Queryable)]
pub struct PhotoPerson {
    pub id: i32,
    pub photo_id: i32,
    pub person_id: i32,
}

use super::schema::people;
#[derive(Debug, Clone, Insertable)]
#[table_name="people"]
pub struct NewPerson<'a> {
    pub person_name: &'a str,
    pub slug: &'a str,
}

use super::schema::photo_people;
#[derive(Debug, Clone, Insertable)]
#[table_name="photo_people"]
pub struct NewPhotoPerson {
    pub photo_id: i32,
    pub person_id: i32,
}

#[derive(Debug, Clone, Queryable)]
pub struct Place {
    pub id: i32,
    pub slug: String,
    pub place_name: String,
}

#[derive(Debug, Clone, Queryable)]
pub struct PhotoPlace {
    pub id: i32,
    pub photo_id: i32,
    pub place_id: i32,
}

use super::schema::places;
#[derive(Debug, Clone, Insertable)]
#[table_name="places"]
pub struct NewPlace<'a> {
    pub slug: &'a str,
    pub place_name: &'a str,
}

use super::schema::photo_places;
#[derive(Debug, Clone, Insertable)]
#[table_name="photo_places"]
pub struct NewPhotoPlace {
    pub photo_id: i32,
    pub place_id: i32,
}

use super::schema::positions;
#[derive(Debug, Clone, Insertable)]
#[table_name="positions"]
pub struct NewPosition {
    pub photo_id: i32,
    pub latitude: i32,
    pub longitude: i32,
}

use super::schema::users;
#[derive(Insertable)]
#[table_name="users"]
pub struct NewUser<'a> {
    pub username: &'a str,
    pub password: &'a str,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
pub struct Camera {
    pub id: i32,
    pub manufacturer: String,
    pub model: String,
}
use super::schema::cameras;
#[derive(Debug, Clone, Insertable)]
#[table_name="cameras"]
pub struct NewCamera {
    pub manufacturer: String,
    pub model: String,
}

impl Camera {
    pub fn get_or_create(db: &PgConnection,
                         make: &str,
                         modl: &str)
                         -> Result<Camera, DieselError> {
        use diesel;
        use diesel::prelude::*;
        use schema::cameras::dsl::*;
        if let Some(camera) = cameras.filter(manufacturer.eq(make))
                                   .filter(model.eq(modl))
                                   .first::<Camera>(db)
                                   .optional()? {
            Ok(camera)
        } else {
            let camera = NewCamera {
                manufacturer: make.to_string(),
                model: modl.to_string(),
            };
            diesel::insert(&camera)
                .into(cameras)
                .get_result(db)
        }
    }
}
