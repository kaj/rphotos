use chrono::naive::datetime::NaiveDateTime;
use rustc_serialize::{Encodable, Encoder};
use diesel::pg::PgConnection;
use diesel::result::Error as DieselError;

#[derive(Debug, Clone, Queryable)]
#[belongs_to(Camera)]
pub struct Photo {
    pub id: i32,
    pub path: String,
    pub date: Option<NaiveDateTime>,
    pub grade: Option<i16>,
    pub rotation: i16,
    pub is_public: bool,
    pub camera_id: Option<i32>,
}

// NaiveDateTime isn't Encodable, so we have to implement this by hand.
impl Encodable for Photo {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        s.emit_struct("Photo", 3, |s| {
            try!(s.emit_struct_field("id", 0, |s| s.emit_i32(self.id)));
            try!(s.emit_struct_field("path", 1, |s| s.emit_str(&self.path)));
            try!(s.emit_struct_field("date", 2, |s| {
                s.emit_str(&self.date
                                .map(|d| format!("{:?}", d))
                                .unwrap_or("-".to_string()))
            }));
            try!(s.emit_struct_field("grade", 2, |s| match self.grade {
                Some(g) => s.emit_option_some(|s| s.emit_i16(g)),
                None => s.emit_option_none(),
            }));
            s.emit_struct_field("rotation", 2, |s| s.emit_i16(self.rotation))
        })
    }
}

use super::schema::photos;
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

    #[allow(dead_code)]
    pub fn query<'a>(auth: bool) -> photos::BoxedQuery<'a, Pg> {
        use super::schema::photos::dsl::{is_public, photos, path};
        use diesel::prelude::*;
        let result = photos
            .filter(path.not_like("%.CR2"))
            .into_boxed();
        if !auth {
            result.filter(is_public)
        } else {
            result
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
        let cameraid = camera.map(|c| c.id);
        if let Some(mut pic) =
               try!(photos.filter(path.eq(&file_path.to_string()))
                          .first::<Photo>(db)
                          .optional()) {
            let mut change = false;
            // TODO Merge updates to one update statement!
            if exifdate.is_some() && exifdate != pic.date {
                change = true;
                pic = try!(diesel::update(photos.find(pic.id))
                               .set(date.eq(exifdate))
                               .get_result::<Photo>(db));
            }
            if exifrotation != pic.rotation {
                change = true;
                pic = try!(diesel::update(photos.find(pic.id))
                               .set(rotation.eq(exifrotation))
                               .get_result::<Photo>(db));
            }
            if cameraid.is_some() && cameraid != pic.camera_id {
                change = true;
                pic = try!(diesel::update(photos.find(pic.id))
                               .set(camera_id.eq(cameraid))
                               .get_result::<Photo>(db));
            }
            Ok(if change {
                Modification::Updated(pic)
            } else {
                Modification::Unchanged(pic)
            })
        } else {
            let pic = NewPhoto {
                path: &file_path,
                date: exifdate,
                rotation: exifrotation,
                camera_id: cameraid,
            };
            let pic = try!(diesel::insert(&pic)
                               .into(photos)
                               .get_result::<Photo>(db));
            Ok(Modification::Created(pic))
        }
    }
}

#[derive(Debug, Clone, RustcEncodable, Queryable)]
pub struct Tag {
    pub id: i32,
    pub slug: String,
    pub tag_name: String,
}


#[derive(Debug, Clone, RustcEncodable, Queryable)]
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


#[derive(Debug, Clone, RustcEncodable, Queryable)]
pub struct Person {
    pub id: i32,
    pub slug: String,
    pub person_name: String,
}

#[derive(Debug, Clone, RustcEncodable, Queryable)]
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

#[derive(Debug, Clone, RustcEncodable, Queryable)]
pub struct Place {
    pub id: i32,
    pub slug: String,
    pub place_name: String,
}

#[derive(Debug, Clone, RustcEncodable, Queryable)]
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

#[derive(Debug, Clone, Identifiable, RustcEncodable, Queryable)]
#[has_many(photos)]
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
        if let Some(camera) = try!(cameras.filter(manufacturer.eq(make))
                                   .filter(model.eq(modl))
                                   .first::<Camera>(db)
                                   .optional()) {
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
