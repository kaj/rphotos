//! Admin-only views, generally called by javascript.
use super::error::ViewResult;
use super::{redirect_to_img, wrap, Context, Result, ViewError};
use crate::models::{Coord, Photo, SizeTag};
use diesel::{self, prelude::*};
use diesel_async::{AsyncPgConnection, RunQueryDsl, SaveChangesDsl};
use serde::Deserialize;
use slug::slugify;
use tracing::{info, warn};
use warp::filters::BoxedFilter;
use warp::http::response::Builder;
use warp::reply::Response;
use warp::Filter;

pub fn routes(s: BoxedFilter<(Context,)>) -> BoxedFilter<(Response,)> {
    use warp::{body::form, path, post};
    let route = path("grade")
        .and(s.clone())
        .and(form())
        .then(set_grade)
        .or(path("locate").and(s.clone()).and(form()).then(set_location))
        .unify()
        .or(path("person").and(s.clone()).and(form()).then(set_person))
        .unify()
        .or(path("rotate").and(s.clone()).and(form()).then(rotate))
        .unify()
        .or(path("tag").and(s).and(form()).then(set_tag))
        .unify()
        .map(wrap);
    post().and(route).boxed()
}

async fn rotate(context: Context, form: RotateForm) -> Result<Response> {
    if !context.is_authorized() {
        return Err(ViewError::PermissionDenied);
    }
    info!("Should rotate #{} by {}", form.image, form.angle);
    use crate::schema::photos::dsl::photos;
    let mut c = context.db().await?;
    let c: &mut AsyncPgConnection = &mut c;
    let mut image =
        or_404q!(photos.find(form.image).first::<Photo>(c).await, context);
    let newvalue = (360 + image.rotation + form.angle) % 360;
    info!("Rotation was {}, setting to {}", image.rotation, newvalue);
    image.rotation = newvalue;
    let image = image.save_changes::<Photo>(c).await?;
    context.clear_cache(&image.cache_key(SizeTag::Small));
    context.clear_cache(&image.cache_key(SizeTag::Medium));
    Builder::new().body("ok".into()).ise()
}

#[derive(Deserialize)]
struct RotateForm {
    image: i32,
    angle: i16,
}

async fn set_tag(context: Context, form: TagForm) -> Result<Response> {
    if !context.is_authorized() {
        return Err(ViewError::PermissionDenied);
    }
    let mut c = context.db().await?;
    use crate::models::Tag;
    let tag = {
        use crate::schema::tags::dsl::*;
        if let Some(tag) = tags
            .filter(tag_name.ilike(&form.tag))
            .first::<Tag>(&mut c)
            .await
            .optional()?
        {
            tag
        } else {
            diesel::insert_into(tags)
                .values((tag_name.eq(&form.tag), slug.eq(&slugify(&form.tag))))
                .get_result::<Tag>(&mut c)
                .await?
        }
    };
    use crate::schema::photo_tags::dsl::*;
    let q = photo_tags
        .filter(photo_id.eq(form.image))
        .filter(tag_id.eq(tag.id))
        .count();
    if q.get_result::<i64>(&mut c).await? > 0 {
        info!("Photo #{} already has {:?}", form.image, form.tag);
    } else {
        info!("Add {:?} on photo #{}!", form.tag, form.image);
        diesel::insert_into(photo_tags)
            .values((photo_id.eq(form.image), tag_id.eq(tag.id)))
            .execute(&mut c)
            .await?;
    }
    Ok(redirect_to_img(form.image))
}

#[derive(Deserialize)]
struct TagForm {
    image: i32,
    tag: String,
}

async fn set_person(context: Context, form: PersonForm) -> Result<Response> {
    if !context.is_authorized() {
        return Err(ViewError::PermissionDenied);
    }
    let mut c = context.db().await?;
    use crate::models::{Person, PhotoPerson};
    let person = Person::get_or_create_name(&mut c, &form.person).await?;
    use crate::schema::photo_people::dsl::*;
    let q = photo_people
        .filter(photo_id.eq(form.image))
        .filter(person_id.eq(person.id));
    if q.first::<PhotoPerson>(&mut c).await.optional()?.is_some() {
        info!("Photo #{} already has {:?}", form.image, person);
    } else {
        info!("Add {:?} on photo #{}!", person, form.image);
        diesel::insert_into(photo_people)
            .values((photo_id.eq(form.image), person_id.eq(person.id)))
            .execute(&mut c)
            .await?;
    }
    Ok(redirect_to_img(form.image))
}

#[derive(Deserialize)]
struct PersonForm {
    image: i32,
    person: String,
}

async fn set_grade(context: Context, form: GradeForm) -> Result<Response> {
    if !context.is_authorized() {
        return Err(ViewError::PermissionDenied);
    }
    if form.grade >= 0 && form.grade <= 100 {
        info!("Should set grade of #{} to {}", form.image, form.grade);
        use crate::schema::photos::dsl::{grade, photos};
        let q =
            diesel::update(photos.find(form.image)).set(grade.eq(form.grade));
        match q.execute(&mut context.db().await?).await? {
            1 => {
                return Ok(redirect_to_img(form.image));
            }
            0 => (),
            n => {
                warn!("Strange, updated {} images with id {}", n, form.image);
            }
        }
        Err(ViewError::NotFound(Some(context)))
    } else {
        info!(
            "Grade {} out of range for image #{}",
            form.grade, form.image
        );
        Err(ViewError::BadRequest("grade out of range"))
    }
}

#[derive(Deserialize)]
struct GradeForm {
    image: i32,
    grade: i16,
}

async fn set_location(context: Context, form: CoordForm) -> Result<Response> {
    if !context.is_authorized() {
        return Err(ViewError::PermissionDenied);
    }
    let image = form.image;
    let coord = form.coord();
    info!("Should set location of #{} to {:?}.", image, coord);

    let (lat, lng) = ((coord.x * 1e6) as i32, (coord.y * 1e6) as i32);
    use crate::schema::positions::dsl::*;
    use diesel::insert_into;
    let mut db = context.db().await?;
    insert_into(positions)
        .values((photo_id.eq(image), latitude.eq(lat), longitude.eq(lng)))
        .on_conflict(photo_id)
        .do_update()
        .set((latitude.eq(lat), longitude.eq(lng)))
        .execute(&mut db)
        .await?;
    match context
        .overpass()
        .update_image_places(&mut db, form.image)
        .await
    {
        Ok(()) => (),
        // Note: We log this error, but don't bother the user.
        Err(err) => warn!("Failed to fetch places: {:?}", err),
    }
    Ok(redirect_to_img(form.image))
}

#[derive(Deserialize)]
struct CoordForm {
    image: i32,
    lat: f64,
    lng: f64,
}

impl CoordForm {
    fn coord(&self) -> Coord {
        Coord {
            x: self.lat,
            y: self.lng,
        }
    }
}
