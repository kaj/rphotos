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
use async_trait::async_trait;
use chrono::naive::NaiveDateTime;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::result::Error;
use diesel::sql_types::Integer;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use slug::slugify;
use std::cmp::max;

pub struct PhotoDetails {
    photo: Photo,
    pub people: Vec<Person>,
    pub places: Vec<Place>,
    pub tags: Vec<Tag>,
    pub pos: Option<Coord>,
    pub attribution: Option<String>,
    pub camera: Option<Camera>,
}
impl PhotoDetails {
    pub async fn load(
        id: i32,
        db: &mut AsyncPgConnection,
    ) -> Result<Self, Error> {
        use crate::schema::photos::dsl::photos;
        let photo = photos.find(id).first::<Photo>(db).await?;
        let attribution = if let Some(id) = photo.attribution_id {
            Some(a::attributions.find(id).select(a::name).first(db).await?)
        } else {
            None
        };
        let camera = if let Some(id) = photo.camera_id {
            Some(c::cameras.find(id).first(db).await?)
        } else {
            None
        };

        Ok(PhotoDetails {
            photo,
            people: h::people
                .filter(
                    h::id.eq_any(
                        ph::photo_people
                            .select(ph::person_id)
                            .filter(ph::photo_id.eq(id)),
                    ),
                )
                .load(db)
                .await?,
            places: l::places
                .filter(
                    l::id.eq_any(
                        pl::photo_places
                            .select(pl::place_id)
                            .filter(pl::photo_id.eq(id)),
                    ),
                )
                .order(l::osm_level.desc().nulls_first())
                .load(db)
                .await?,
            tags: t::tags
                .filter(
                    t::id.eq_any(
                        pt::photo_tags
                            .select(pt::tag_id)
                            .filter(pt::photo_id.eq(id)),
                    ),
                )
                .load(db)
                .await?,
            pos: pos::positions
                .filter(pos::photo_id.eq(id))
                .select((pos::latitude, pos::longitude))
                .first(db)
                .await
                .optional()?,
            attribution,
            camera,
        })
    }
}

impl std::ops::Deref for PhotoDetails {
    type Target = Photo;
    fn deref(&self) -> &Photo {
        &self.photo
    }
}

#[derive(AsChangeset, Clone, Debug, Identifiable, Queryable, Selectable)]
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
        if auth {
            result
        } else {
            result.filter(p::is_public)
        }
    }

    pub async fn update_by_path(
        db: &mut AsyncPgConnection,
        file_path: &str,
        newwidth: i32,
        newheight: i32,
        exifdate: Option<NaiveDateTime>,
        camera: &Option<Camera>,
    ) -> Result<Option<Modification<Photo>>, Error> {
        if let Some(mut pic) = p::photos
            .filter(p::path.eq(&file_path.to_string()))
            .first::<Photo>(db)
            .await
            .optional()?
        {
            let mut change = false;
            // TODO Merge updates to one update statement!
            if pic.width != newwidth || pic.height != newheight {
                change = true;
                pic = diesel::update(p::photos.find(pic.id))
                    .set((p::width.eq(newwidth), p::height.eq(newheight)))
                    .get_result::<Photo>(db)
                    .await?;
            }
            if exifdate.is_some() && exifdate != pic.date {
                change = true;
                pic = diesel::update(p::photos.find(pic.id))
                    .set(p::date.eq(exifdate))
                    .get_result::<Photo>(db)
                    .await?;
            }
            if let Some(ref camera) = *camera {
                if pic.camera_id != Some(camera.id) {
                    change = true;
                    pic = diesel::update(p::photos.find(pic.id))
                        .set(p::camera_id.eq(camera.id))
                        .get_result::<Photo>(db)
                        .await?;
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

    pub async fn create_or_set_basics(
        db: &mut AsyncPgConnection,
        file_path: &str,
        newwidth: i32,
        newheight: i32,
        exifdate: Option<NaiveDateTime>,
        exifrotation: i16,
        camera: Option<Camera>,
    ) -> Result<Modification<Photo>, Error> {
        if let Some(result) = Self::update_by_path(
            db, file_path, newwidth, newheight, exifdate, &camera,
        )
        .await?
        {
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
                .get_result::<Photo>(db)
                .await?;
            Ok(Modification::Created(pic))
        }
    }

    pub fn get_size(&self, size: SizeTag) -> (u32, u32) {
        let (width, height) = (self.width, self.height);
        let scale = f64::from(size.px()) / f64::from(max(width, height));
        let w = (scale * f64::from(width)) as u32;
        let h = (scale * f64::from(height)) as u32;
        match self.rotation {
            _x @ (0..=44 | 315..=360 | 135..=224) => (w, h),
            _ => (h, w),
        }
    }

    #[cfg(test)]
    pub fn mock(y: i32, mo: u32, da: u32, h: u32, m: u32, s: u32) -> Self {
        use chrono::naive::NaiveDate;
        Photo {
            id: ((((((y as u32 * 12) + mo) * 30 + da) * 24) + h) * 60 + s)
                as i32,
            path: format!("{y}/{mo:02}/{da:02}/IMG{h:02}{m:02}{s:02}.jpg"),
            date: NaiveDate::from_ymd_opt(y, mo, da)
                .unwrap()
                .and_hms_opt(h, m, s),
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

#[async_trait]
pub trait Facet {
    async fn load_slugs(
        slugs: &[String],
        db: &mut AsyncPgConnection,
    ) -> Result<Vec<Self>, Error>
    where
        Self: Sized;
}

#[derive(Debug, Clone, Queryable)]
pub struct Tag {
    pub id: i32,
    pub slug: String,
    pub tag_name: String,
}

#[async_trait]
impl Facet for Tag {
    async fn load_slugs(
        slugs: &[String],
        db: &mut AsyncPgConnection,
    ) -> Result<Vec<Tag>, Error> {
        t::tags.filter(t::slug.eq_any(slugs)).load(db).await
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
    pub async fn get_or_create_name(
        db: &mut AsyncPgConnection,
        name: &str,
    ) -> Result<Person, Error> {
        if let Some(name) = h::people
            .filter(h::person_name.ilike(name))
            .first(db)
            .await
            .optional()?
        {
            Ok(name)
        } else {
            diesel::insert_into(h::people)
                .values((h::person_name.eq(name), h::slug.eq(&slugify(name))))
                .get_result(db)
                .await
        }
    }
}

#[async_trait]
impl Facet for Person {
    async fn load_slugs(
        slugs: &[String],
        db: &mut AsyncPgConnection,
    ) -> Result<Vec<Person>, Error> {
        h::people.filter(h::slug.eq_any(slugs)).load(db).await
    }
}

#[derive(Debug, Clone, Queryable)]
pub struct PhotoPerson {
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

#[async_trait]
impl Facet for Place {
    async fn load_slugs(
        slugs: &[String],
        db: &mut AsyncPgConnection,
    ) -> Result<Vec<Place>, Error> {
        l::places.filter(l::slug.eq_any(slugs)).load(db).await
    }
}

#[derive(Debug, Clone, Queryable)]
pub struct PhotoPlace {
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
    pub async fn get_or_create(
        db: &mut AsyncPgConnection,
        make: &str,
        modl: &str,
    ) -> Result<Camera, Error> {
        if let Some(camera) = c::cameras
            .filter(c::manufacturer.eq(make))
            .filter(c::model.eq(modl))
            .first::<Camera>(db)
            .await
            .optional()?
        {
            Ok(camera)
        } else {
            diesel::insert_into(c::cameras)
                .values((c::manufacturer.eq(make), c::model.eq(modl)))
                .get_result(db)
                .await
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

    fn build(row: Self::Row) -> diesel::deserialize::Result<Self> {
        Ok(Coord::from((row.0, row.1)))
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
    pub fn px(self) -> u16 {
        match self {
            SizeTag::Small => 288,
            SizeTag::Medium => 1080,
            SizeTag::Large => 8192, // not really used
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
