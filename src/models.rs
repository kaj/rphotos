use crate::schema::attributions::dsl as a;
use crate::schema::cameras;
use crate::schema::cameras::dsl as c;
use crate::schema::people::dsl as h;
use crate::schema::photo_people::dsl as ph;
use crate::schema::photo_places::dsl as pl;
use crate::schema::photo_tags::dsl as pt;
use crate::schema::photos;
use crate::schema::photos::dsl as p;
use crate::schema::places::dsl as l;
use crate::schema::positions::dsl as pos;
use crate::schema::tags::dsl as t;
use chrono::naive::NaiveDateTime;
use diesel;
use diesel::pg::{Pg, PgConnection};
use diesel::prelude::*;
use diesel::result::Error;
use diesel::sql_types::Integer;
use log::error;
use slug::slugify;
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
    pub width: i32,
    pub height: i32,
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
        self.is_public
    }

    pub fn cache_key(&self, size: SizeTag) -> String {
        format!("rp{}{:?}", self.id, size)
    }

    #[allow(dead_code)]
    pub fn query<'a>(auth: bool) -> photos::BoxedQuery<'a, Pg> {
        let result = p::photos
            .filter(p::path.not_like("%.CR2"))
            .filter(p::path.not_like("%.dng"))
            .into_boxed();
        if !auth {
            result.filter(p::is_public)
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
    ) -> Result<Option<Modification<Photo>>, Error> {
        if let Some(mut pic) = p::photos
            .filter(p::path.eq(&file_path.to_string()))
            .first::<Photo>(db)
            .optional()?
        {
            let mut change = false;
            // TODO Merge updates to one update statement!
            if pic.width != newwidth || pic.height != newheight {
                change = true;
                pic = diesel::update(p::photos.find(pic.id))
                    .set((p::width.eq(newwidth), p::height.eq(newheight)))
                    .get_result::<Photo>(db)?;
            }
            if exifdate.is_some() && exifdate != pic.date {
                change = true;
                pic = diesel::update(p::photos.find(pic.id))
                    .set(p::date.eq(exifdate))
                    .get_result::<Photo>(db)?;
            }
            if let Some(ref camera) = *camera {
                if pic.camera_id != Some(camera.id) {
                    change = true;
                    pic = diesel::update(p::photos.find(pic.id))
                        .set(p::camera_id.eq(camera.id))
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
    ) -> Result<Modification<Photo>, Error> {
        if let Some(result) = Self::update_by_path(
            db, file_path, newwidth, newheight, exifdate, &camera,
        )? {
            Ok(result)
        } else {
            let pic = diesel::insert_into(p::photos)
                .values((
                    p::path.eq(file_path),
                    p::date.eq(exifdate),
                    p::rotation.eq(exifrotation),
                    p::width.eq(newwidth),
                    p::height.eq(newheight),
                    p::camera_id.eq(camera.map(|c| c.id)),
                ))
                .get_result::<Photo>(db)?;
            Ok(Modification::Created(pic))
        }
    }

    pub fn load_people(
        &self,
        db: &PgConnection,
    ) -> Result<Vec<Person>, Error> {
        h::people
            .filter(
                h::id.eq_any(
                    ph::photo_people
                        .select(ph::person_id)
                        .filter(ph::photo_id.eq(self.id)),
                ),
            )
            .load(db)
    }

    pub fn load_places(&self, db: &PgConnection) -> Result<Vec<Place>, Error> {
        l::places
            .filter(
                l::id.eq_any(
                    pl::photo_places
                        .select(pl::place_id)
                        .filter(pl::photo_id.eq(self.id)),
                ),
            )
            .order(l::osm_level.desc().nulls_first())
            .load(db)
    }
    pub fn load_tags(&self, db: &PgConnection) -> Result<Vec<Tag>, Error> {
        t::tags
            .filter(
                t::id.eq_any(
                    pt::photo_tags
                        .select(pt::tag_id)
                        .filter(pt::photo_id.eq(self.id)),
                ),
            )
            .load(db)
    }

    pub fn load_position(&self, db: &PgConnection) -> Option<Coord> {
        match pos::positions
            .filter(pos::photo_id.eq(self.id))
            .select((pos::latitude, pos::longitude))
            .first::<(i32, i32)>(db)
        {
            Ok(c) => Some(c.into()),
            Err(diesel::NotFound) => None,
            Err(err) => {
                error!("Failed to read position: {}", err);
                None
            }
        }
    }
    pub fn load_attribution(&self, db: &PgConnection) -> Option<String> {
        self.attribution_id.and_then(|i| {
            a::attributions.find(i).select(a::name).first(db).ok()
        })
    }
    pub fn load_camera(&self, db: &PgConnection) -> Option<Camera> {
        self.camera_id
            .and_then(|i| c::cameras.find(i).first(db).ok())
    }
    pub fn get_size(&self, size: SizeTag) -> (u32, u32) {
        let (width, height) = (self.width, self.height);
        let scale = f64::from(size.px()) / f64::from(max(width, height));
        let w = (scale * f64::from(width)) as u32;
        let h = (scale * f64::from(height)) as u32;
        match self.rotation {
            _x @ 0..=44 | _x @ 315..=360 | _x @ 135..=224 => (w, h),
            _ => (h, w),
        }
    }

    #[cfg(test)]
    pub fn mock(y: i32, mo: u32, da: u32, h: u32, m: u32, s: u32) -> Self {
        use chrono::naive::NaiveDate;
        Photo {
            id: ((((((y as u32 * 12) + mo) * 30 + da) * 24) + h) * 60 + s)
                as i32,
            path: format!(
                "{}/{:02}/{:02}/IMG{:02}{:02}{:02}.jpg",
                y, mo, da, h, m, s,
            ),
            date: Some(NaiveDate::from_ymd(y, mo, da).and_hms(h, m, s)),
            grade: None,
            rotation: 0,
            is_public: false,
            camera_id: None,
            attribution_id: None,
            width: 4000,
            height: 3000,
        }
    }
}

pub trait Facet {
    fn by_slug(slug: &str, db: &PgConnection) -> Result<Self, Error>
    where
        Self: Sized;
}

#[derive(Debug, Clone, Queryable)]
pub struct Tag {
    pub id: i32,
    pub slug: String,
    pub tag_name: String,
}

impl Facet for Tag {
    fn by_slug(slug: &str, db: &PgConnection) -> Result<Tag, Error> {
        t::tags.filter(t::slug.eq(slug)).first(db)
    }
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

impl Person {
    pub fn get_or_create_name(
        db: &PgConnection,
        name: &str,
    ) -> Result<Person, Error> {
        h::people
            .filter(h::person_name.ilike(name))
            .first(db)
            .or_else(|_| {
                diesel::insert_into(h::people)
                    .values((
                        h::person_name.eq(name),
                        h::slug.eq(&slugify(name)),
                    ))
                    .get_result(db)
            })
    }
}

impl Facet for Person {
    fn by_slug(slug: &str, db: &PgConnection) -> Result<Person, Error> {
        h::people.filter(h::slug.eq(slug)).first(db)
    }
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
    pub osm_id: Option<i64>,
    pub osm_level: Option<i16>,
}

impl Facet for Place {
    fn by_slug(slug: &str, db: &PgConnection) -> Result<Place, Error> {
        l::places.filter(l::slug.eq(slug)).first(db)
    }
}

#[derive(Debug, Clone, Queryable)]
pub struct PhotoPlace {
    pub id: i32,
    pub photo_id: i32,
    pub place_id: i32,
}

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
    ) -> Result<Camera, Error> {
        if let Some(camera) = c::cameras
            .filter(c::manufacturer.eq(make))
            .filter(c::model.eq(modl))
            .first::<Camera>(db)
            .optional()?
        {
            Ok(camera)
        } else {
            diesel::insert_into(c::cameras)
                .values((c::manufacturer.eq(make), c::model.eq(modl)))
                .get_result(db)
        }
    }
}

#[derive(Debug, Clone)]
pub struct Coord {
    pub x: f64,
    pub y: f64,
}

impl Queryable<(Integer, Integer), Pg> for Coord {
    type Row = (i32, i32);

    fn build(row: Self::Row) -> Self {
        Coord::from((row.0, row.1))
    }
}

impl From<(i32, i32)> for Coord {
    fn from((lat, long): (i32, i32)) -> Coord {
        Coord {
            x: f64::from(lat) / 1e6,
            y: f64::from(long) / 1e6,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeTag {
    Small,
    Medium,
    Large,
}

impl SizeTag {
    pub fn px(self) -> u32 {
        match self {
            SizeTag::Small => 240,
            SizeTag::Medium => 960,
            SizeTag::Large => 1900,
        }
    }
    pub fn tag(self) -> char {
        match self {
            SizeTag::Small => 's',
            SizeTag::Medium => 'm',
            SizeTag::Large => 'l',
        }
    }
}
