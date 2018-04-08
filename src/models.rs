use chrono::naive::NaiveDateTime;
use diesel;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use server::SizeTag;
use std::cmp::max;

#[derive(AsChangeset, Clone, Debug, Identifiable, Queryable)]
pub struct Photo {
    pub id: i32,
    pub path: String,
    pub date: Option<NaiveDateTime>,
    pub grade: Option<i16>,
    pub rotation: i16,
    pub is_public: bool,
    pub camera_id: Option<i32>,
    pub attribution_id: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

use schema::photos;

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
        use super::schema::photos::dsl::{is_public, path, photos};
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

    pub fn update_by_path(
        db: &PgConnection,
        file_path: &str,
        newwidth: i32,
        newheight: i32,
        exifdate: Option<NaiveDateTime>,
        camera: &Option<Camera>,
    ) -> Result<Option<Modification<Photo>>, DieselError> {
        use diesel;
        use diesel::prelude::*;
        use schema::photos::dsl::*;
        if let Some(mut pic) = photos
            .filter(path.eq(&file_path.to_string()))
            .first::<Photo>(db)
            .optional()?
        {
            let mut change = false;
            // TODO Merge updates to one update statement!
            if pic.width != Some(newwidth) || pic.height != Some(newheight) {
                change = true;
                pic = diesel::update(photos.find(pic.id))
                    .set((width.eq(newwidth), height.eq(newheight)))
                    .get_result::<Photo>(db)?;
            }
            if exifdate.is_some() && exifdate != pic.date {
                change = true;
                pic = diesel::update(photos.find(pic.id))
                    .set(date.eq(exifdate))
                    .get_result::<Photo>(db)?;
            }
            if let Some(ref camera) = *camera {
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

    pub fn create_or_set_basics(
        db: &PgConnection,
        file_path: &str,
        newwidth: i32,
        newheight: i32,
        exifdate: Option<NaiveDateTime>,
        exifrotation: i16,
        camera: Option<Camera>,
    ) -> Result<Modification<Photo>, DieselError> {
        use diesel;
        use diesel::prelude::*;
        use schema::photos::dsl::*;
        if let Some(result) = Self::update_by_path(
            db,
            file_path,
            newwidth,
            newheight,
            exifdate,
            &camera,
        )? {
            Ok(result)
        } else {
            let pic = diesel::insert_into(photos)
                .values((
                    path.eq(file_path),
                    date.eq(exifdate),
                    rotation.eq(exifrotation),
                    width.eq(newwidth),
                    height.eq(newheight),
                    camera_id.eq(camera.map(|c| c.id)),
                ))
                .get_result::<Photo>(db)?;
            Ok(Modification::Created(pic))
        }
    }

    pub fn load_people(
        &self,
        db: &PgConnection,
    ) -> Result<Vec<Person>, DieselError> {
        use schema::people::dsl::{id, people};
        use schema::photo_people::dsl::{person_id, photo_id, photo_people};
        people
            .filter(
                id.eq_any(
                    photo_people
                        .select(person_id)
                        .filter(photo_id.eq(self.id)),
                ),
            )
            .load(db)
    }

    pub fn load_places(
        &self,
        db: &PgConnection,
    ) -> Result<Vec<Place>, DieselError> {
        use schema::photo_places::dsl::{photo_id, photo_places, place_id};
        use schema::places::dsl::{id, places};
        places
            .filter(
                id.eq_any(
                    photo_places
                        .select(place_id)
                        .filter(photo_id.eq(self.id)),
                ),
            )
            .load(db)
    }
    pub fn load_tags(
        &self,
        db: &PgConnection,
    ) -> Result<Vec<Tag>, DieselError> {
        use schema::photo_tags::dsl::{photo_id, photo_tags, tag_id};
        use schema::tags::dsl::{id, tags};
        tags.filter(
            id.eq_any(
                photo_tags
                    .select(tag_id)
                    .filter(photo_id.eq(self.id)),
            ),
        ).load(db)
    }

    pub fn load_position(&self, db: &PgConnection) -> Option<Coord> {
        use schema::positions::dsl::*;
        match positions
            .filter(photo_id.eq(self.id))
            .select((latitude, longitude))
            .first::<(i32, i32)>(db)
        {
            Ok((tlat, tlong)) => Some(Coord {
                x: f64::from(tlat) / 1e6,
                y: f64::from(tlong) / 1e6,
            }),
            Err(diesel::NotFound) => None,
            Err(err) => {
                error!("Failed to read position: {}", err);
                None
            }
        }
    }
    pub fn load_attribution(&self, db: &PgConnection) -> Option<String> {
        use schema::attributions::dsl::*;
        self.attribution_id
            .and_then(|i| attributions.find(i).select(name).first(db).ok())
    }
    pub fn load_camera(&self, db: &PgConnection) -> Option<Camera> {
        use schema::cameras::dsl::cameras;
        self.camera_id
            .and_then(|i| cameras.find(i).first(db).ok())
    }
    pub fn get_size(&self, max_size: u32) -> Option<(u32, u32)> {
        if let (Some(width), Some(height)) = (self.width, self.height) {
            let scale = f64::from(max_size) / f64::from(max(width, height));
            let w = (scale * f64::from(width)) as u32;
            let h = (scale * f64::from(height)) as u32;
            match self.rotation {
                _x @ 0...44 | _x @ 315...360 | _x @ 135...224 => Some((w, h)),
                _ => Some((h, w)),
            }
        } else {
            None
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

use super::schema::cameras;
#[derive(Debug, Clone, Identifiable, Queryable)]
pub struct Camera {
    pub id: i32,
    pub manufacturer: String,
    pub model: String,
}

impl Camera {
    pub fn get_or_create(
        db: &PgConnection,
        make: &str,
        modl: &str,
    ) -> Result<Camera, DieselError> {
        use diesel;
        use diesel::prelude::*;
        use schema::cameras::dsl::*;
        if let Some(camera) = cameras
            .filter(manufacturer.eq(make))
            .filter(model.eq(modl))
            .first::<Camera>(db)
            .optional()?
        {
            Ok(camera)
        } else {
            diesel::insert_into(cameras)
                .values((manufacturer.eq(make), model.eq(modl)))
                .get_result(db)
        }
    }
}

#[derive(Debug, Clone)]
pub struct Coord {
    pub x: f64,
    pub y: f64,
}
