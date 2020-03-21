//! Admin-only views, generally called by javascript.
use super::{not_found, permission_denied, redirect_to_img, Context};
use crate::models::{Coord, Photo, SizeTag};
use diesel::{self, prelude::*};
use log::{info, warn};
use serde::Deserialize;
use slug::slugify;
use warp::filters::BoxedFilter;
use warp::http::response::Builder;
use warp::reply::Response;
use warp::{Filter, Rejection, Reply};

pub fn routes(s: BoxedFilter<(Context,)>) -> BoxedFilter<(impl Reply,)> {
    use warp::{body::form, path, post};
    let route = path("grade")
        .and(s.clone())
        .and(form())
        .and_then(set_grade)
        .or(path("locate")
            .and(s.clone())
            .and(form())
            .and_then(set_location))
        .unify()
        .or(path("person")
            .and(s.clone())
            .and(form())
            .and_then(set_person))
        .unify()
        .or(path("rotate").and(s.clone()).and(form()).map(rotate))
        .unify()
        .or(path("tag").and(s).and(form()).and_then(set_tag))
        .unify();
    post().and(route).boxed()
}

fn rotate(context: Context, form: RotateForm) -> Response {
    if !context.is_authorized() {
        return permission_denied().unwrap();
    }
    info!("Should rotate #{} by {}", form.image, form.angle);
    use crate::schema::photos::dsl::photos;
    let c = context.db().unwrap();
    let c: &PgConnection = &c;
    if let Ok(mut image) = photos.find(form.image).first::<Photo>(c) {
        let newvalue = (360 + image.rotation + form.angle) % 360;
        info!("Rotation was {}, setting to {}", image.rotation, newvalue);
        image.rotation = newvalue;
        match image.save_changes::<Photo>(c) {
            Ok(image) => {
                context.clear_cache(&image.cache_key(SizeTag::Small));
                context.clear_cache(&image.cache_key(SizeTag::Medium));
                return Builder::new().body("ok".into()).unwrap();
            }
            Err(error) => {
                warn!("Failed to save image #{}: {}", image.id, error);
            }
        }
    }
    not_found(&context)
}

#[derive(Deserialize)]
struct RotateForm {
    image: i32,
    angle: i16,
}

type WarpResult = Result<Response, Rejection>;

async fn set_tag(context: Context, form: TagForm) -> WarpResult {
    if !context.is_authorized() {
        return permission_denied();
    }
    let c = context.db().unwrap();
    use crate::models::{PhotoTag, Tag};
    let tag = {
        use crate::schema::tags::dsl::*;
        tags.filter(tag_name.ilike(&form.tag))
            .first::<Tag>(&c)
            .or_else(|_| {
                diesel::insert_into(tags)
                    .values((
                        tag_name.eq(&form.tag),
                        slug.eq(&slugify(&form.tag)),
                    ))
                    .get_result::<Tag>(&c)
            })
            .expect("Find or create tag")
    };
    use crate::schema::photo_tags::dsl::*;
    let q = photo_tags
        .filter(photo_id.eq(form.image))
        .filter(tag_id.eq(tag.id));
    if q.first::<PhotoTag>(&c).is_ok() {
        info!("Photo #{} already has {:?}", form.image, form.tag);
    } else {
        info!("Add {:?} on photo #{}!", form.tag, form.image);
        diesel::insert_into(photo_tags)
            .values((photo_id.eq(form.image), tag_id.eq(tag.id)))
            .execute(&c)
            .expect("Tag a photo");
    }
    Ok(redirect_to_img(form.image))
}

#[derive(Deserialize)]
struct TagForm {
    image: i32,
    tag: String,
}

async fn set_person(context: Context, form: PersonForm) -> WarpResult {
    if !context.is_authorized() {
        return permission_denied();
    }
    let c = context.db().unwrap();
    use crate::models::{Person, PhotoPerson};
    let person = Person::get_or_create_name(&c, &form.person)
        .expect("Find or create person");
    use crate::schema::photo_people::dsl::*;
    let q = photo_people
        .filter(photo_id.eq(form.image))
        .filter(person_id.eq(person.id));
    if q.first::<PhotoPerson>(&c).is_ok() {
        info!("Photo #{} already has {:?}", form.image, person);
    } else {
        info!("Add {:?} on photo #{}!", person, form.image);
        diesel::insert_into(photo_people)
            .values((photo_id.eq(form.image), person_id.eq(person.id)))
            .execute(&c)
            .expect("Name person in photo");
    }
    Ok(redirect_to_img(form.image))
}

#[derive(Deserialize)]
struct PersonForm {
    image: i32,
    person: String,
}

async fn set_grade(context: Context, form: GradeForm) -> WarpResult {
    if !context.is_authorized() {
        return permission_denied();
    }
    if form.grade >= 0 && form.grade <= 100 {
        info!("Should set grade of #{} to {}", form.image, form.grade);
        use crate::schema::photos::dsl::{grade, photos};
        let q =
            diesel::update(photos.find(form.image)).set(grade.eq(form.grade));
        match q.execute(&context.db().unwrap()) {
            Ok(1) => {
                return Ok(redirect_to_img(form.image));
            }
            Ok(0) => (),
            Ok(n) => {
                warn!("Strange, updated {} images with id {}", n, form.image);
            }
            Err(error) => {
                warn!("Failed set grade of image #{}: {}", form.image, error);
            }
        }
    } else {
        info!(
            "Grade {} out of range for image #{}",
            form.grade, form.image
        );
    }
    Ok(not_found(&context))
}

#[derive(Deserialize)]
struct GradeForm {
    image: i32,
    grade: i16,
}

async fn set_location(context: Context, form: CoordForm) -> WarpResult {
    if !context.is_authorized() {
        return permission_denied();
    }
    let image = form.image;
    let coord = form.coord();
    info!("Should set location of #{} to {:?}.", image, coord);

    let (lat, lng) = ((coord.x * 1e6) as i32, (coord.y * 1e6) as i32);
    use crate::schema::positions::dsl::*;
    use diesel::insert_into;
    insert_into(positions)
        .values((photo_id.eq(image), latitude.eq(lat), longitude.eq(lng)))
        .on_conflict(photo_id)
        .do_update()
        .set((latitude.eq(lat), longitude.eq(lng)))
        .execute(&context.db().unwrap())
        .expect("Insert image position");
    match context
        .overpass()
        .update_image_places(&context.db_pool(), form.image)
        .await
    {
        Ok(()) => (),
        // TODO Tell the user something failed?
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
