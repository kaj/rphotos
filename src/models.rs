use chrono::naive::datetime::NaiveDateTime;
use rustc_serialize::{Encodable, Encoder};
use diesel::pg::PgConnection;
use diesel::result::Error as DieselError;

pub const MIN_PUBLIC_GRADE: i16 = 4;

#[derive(Debug, Clone, Queryable)]
pub struct Photo {
    pub id: i32,
    pub path: String,
    pub date: Option<NaiveDateTime>,
    pub grade: Option<i16>,
    pub rotation: i16,
}

// NaiveDateTime isn't Encodable, so we have to implement this by hand.
impl Encodable for Photo {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        s.emit_struct("Photo", 3, |s| {
            try!(s.emit_struct_field("id", 0, |s| s.emit_i32(self.id)));
            try!(s.emit_struct_field("path", 1, |s| s.emit_str(&self.path)));
            try!(s.emit_struct_field("date", 2, |s|
                s.emit_str(&self.date.map(|d|format!("{:?}", d))
                           .unwrap_or("-".to_string()))
            ));
            try!(s.emit_struct_field("grade", 2, |s| match self.grade {
                Some(g) => s.emit_option_some(|s| s.emit_i16(g)),
                None => s.emit_option_none(),
            }));
            s.emit_struct_field("rotation", 2, |s| s.emit_i16(self.rotation))
        })
    }
}

use super::schema::photos;
#[insertable_into(photos)]
#[derive(Debug, Clone)]
pub struct NewPhoto<'a> {
    pub path: &'a str,
    pub date: Option<NaiveDateTime>,
    pub rotation: i16,
}

#[derive(Debug)]
pub enum Modification<T> {
    Created(T),
    Updated(T),
    Unchanged(T),
}

impl Photo {
    #[allow(dead_code)]
    pub fn is_public(&self) -> bool {
        if let Some(grade) = self.grade {
            grade >= MIN_PUBLIC_GRADE
        } else {
            false
        }
    }

    pub fn create_or_set_basics(db: &PgConnection, file_path: &str,
                                exifdate: Option<NaiveDateTime>, exifrotation: i16)
                                -> Result<Modification<Photo>, DieselError> {
        use diesel;
        use diesel::prelude::*;
        use schema::photos::dsl::*;
        if let Some(mut pic) =
            try!(photos.filter(path.eq(&file_path.to_string()))
                       .first::<Photo>(db)
                       .optional()) {
            let mut change = false;
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
            Ok(if change { Modification::Updated(pic) }
               else { Modification::Unchanged(pic) })
        } else {
            let pic = NewPhoto {
                path: &file_path,
                date: exifdate,
                rotation: exifrotation,
            };
            let pic = try!(diesel::insert(&pic).into(photos)
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
#[insertable_into(tags)]
#[derive(Debug, Clone)]
pub struct NewTag<'a> {
    pub tag_name: &'a str,
    pub slug: &'a str,
}

use super::schema::photo_tags;
#[insertable_into(photo_tags)]
#[derive(Debug, Clone)]
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
#[insertable_into(people)]
#[derive(Debug, Clone)]
pub struct NewPerson<'a> {
    pub person_name: &'a str,
    pub slug: &'a str,
}

use super::schema::photo_people;
#[insertable_into(photo_people)]
#[derive(Debug, Clone)]
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
#[insertable_into(places)]
#[derive(Debug, Clone)]
pub struct NewPlace<'a> {
    pub slug: &'a str,
    pub place_name: &'a str,
}

use super::schema::photo_places;
#[insertable_into(photo_places)]
#[derive(Debug, Clone)]
pub struct NewPhotoPlace {
    pub photo_id: i32,
    pub place_id: i32,
}

use super::schema::positions;
#[insertable_into(positions)]
#[derive(Debug, Clone)]
pub struct NewPosition {
    pub photo_id: i32,
    pub latitude: i32,
    pub longitude: i32,
}

use super::schema::users;
#[insertable_into(users)]
pub struct NewUser<'a> {
    pub username: &'a str,
    pub password: &'a str,
}
