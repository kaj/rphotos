//! Admin-only views, generally called by javascript.
use super::SizeTag;
use diesel::prelude::*;
use memcachemiddleware::MemcacheRequestExtensions;
use models::{Coord, Photo};
use nickel::extensions::Redirect;
use nickel::status::StatusCode;
use nickel::{BodyError, FormBody, MiddlewareResult, Request, Response};
use nickel_diesel::DieselRequestExtensions;
use nickel_jwt_session::SessionRequestExtensions;
use reqwest::Client;
use rustc_serialize::json::Json;
use server::nickelext::MyResponse;
use slug::slugify;

pub fn rotate<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    if req.authorized_user().is_none() {
        return res.error(StatusCode::Unauthorized, "permission denied");
    }
    if let (Some(image), Some(angle)) = try_with!(res, rotate_params(req)) {
        info!("Should rotate #{} by {}", image, angle);
        use schema::photos::dsl::photos;
        let c: &PgConnection = &req.db_conn();
        if let Ok(mut image) = photos.find(image).first::<Photo>(c) {
            let newvalue = (360 + image.rotation + angle) % 360;
            info!("Rotation was {}, setting to {}", image.rotation, newvalue);
            image.rotation = newvalue;
            match image.save_changes::<Photo>(c) {
                Ok(image) => {
                    req.clear_cache(&image.cache_key(SizeTag::Small));
                    req.clear_cache(&image.cache_key(SizeTag::Medium));
                    return res.ok(|o| writeln!(o, "ok"));
                }
                Err(error) => {
                    warn!("Failed to save image #{}: {}", image.id, error);
                }
            }
        }
    }
    info!("Missing image and/or angle to rotate or image not found");
    res.not_found("")
}

type QResult<T> = Result<T, (StatusCode, BodyError)>;

fn rotate_params(req: &mut Request) -> QResult<(Option<i32>, Option<i16>)> {
    let data = req.form_body()?;
    Ok((
        data.get("image").and_then(|s| s.parse().ok()),
        data.get("angle").and_then(|s| s.parse().ok()),
    ))
}

pub fn set_tag<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    if req.authorized_user().is_none() {
        return res.error(StatusCode::Unauthorized, "permission denied");
    }
    if let (Some(image), Some(tag)) = try_with!(res, tag_params(req)) {
        let c: &PgConnection = &req.db_conn();
        use diesel;
        use models::{PhotoTag, Tag};
        let tag = {
            use schema::tags::dsl::*;
            tags.filter(tag_name.ilike(&tag))
                .first::<Tag>(c)
                .or_else(|_| {
                    diesel::insert_into(tags)
                        .values((tag_name.eq(&tag), slug.eq(&slugify(&tag))))
                        .get_result::<Tag>(c)
                }).expect("Find or create tag")
        };
        use schema::photo_tags::dsl::*;
        let q = photo_tags
            .filter(photo_id.eq(image))
            .filter(tag_id.eq(tag.id));
        if q.first::<PhotoTag>(c).is_ok() {
            info!("Photo #{} already has {:?}", image, tag);
        } else {
            info!("Add {:?} on photo #{}!", tag, image);
            diesel::insert_into(photo_tags)
                .values((photo_id.eq(image), tag_id.eq(tag.id)))
                .execute(c)
                .expect("Tag a photo");
        }
        return res.redirect(format!("/img/{}", image));
    }
    info!("Missing image and/or angle to rotate or image not found");
    res.not_found("")
}

fn tag_params(req: &mut Request) -> QResult<(Option<i32>, Option<String>)> {
    let data = req.form_body()?;
    Ok((
        data.get("image").and_then(|s| s.parse().ok()),
        data.get("tag").map(String::from),
    ))
}

pub fn set_person<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    if req.authorized_user().is_none() {
        return res.error(StatusCode::Unauthorized, "permission denied");
    }
    if let (Some(image), Some(name)) = try_with!(res, person_params(req)) {
        let c: &PgConnection = &req.db_conn();
        use diesel;
        use models::{Person, PhotoPerson};
        let person = {
            use schema::people::dsl::*;
            people
                .filter(person_name.ilike(&name))
                .first::<Person>(c)
                .or_else(|_| {
                    diesel::insert_into(people)
                        .values((
                            person_name.eq(&name),
                            slug.eq(&slugify(&name)),
                        )).get_result::<Person>(c)
                }).expect("Find or create tag")
        };
        use schema::photo_people::dsl::*;
        let q = photo_people
            .filter(photo_id.eq(image))
            .filter(person_id.eq(person.id));
        if q.first::<PhotoPerson>(c).is_ok() {
            info!("Photo #{} already has {:?}", image, person);
        } else {
            info!("Add {:?} on photo #{}!", person, image);
            diesel::insert_into(photo_people)
                .values((photo_id.eq(image), person_id.eq(person.id)))
                .execute(c)
                .expect("Name person in photo");
        }
        return res.redirect(format!("/img/{}", image));
    }
    info!("Missing image and/or angle to rotate or image not found");
    res.not_found("")
}

fn person_params(req: &mut Request) -> QResult<(Option<i32>, Option<String>)> {
    let data = req.form_body()?;
    Ok((
        data.get("image").and_then(|s| s.parse().ok()),
        data.get("person").map(String::from),
    ))
}

pub fn set_grade<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    if req.authorized_user().is_none() {
        return res.error(StatusCode::Unauthorized, "permission denied");
    }
    if let (Some(image), Some(newgrade)) = try_with!(res, grade_params(req)) {
        if newgrade >= 0 && newgrade <= 100 {
            info!("Should set grade of #{} to {}", image, newgrade);
            use diesel;
            use schema::photos::dsl::{grade, photos};
            let c: &PgConnection = &req.db_conn();
            let q = diesel::update(photos.find(image)).set(grade.eq(newgrade));
            match q.execute(c) {
                Ok(1) => {
                    return res.redirect(format!("/img/{}", image));
                }
                Ok(0) => (),
                Ok(n) => {
                    warn!("Strange, updated {} images with id {}", n, image);
                }
                Err(error) => {
                    warn!("Failed set grade of image #{}: {}", image, error);
                }
            }
        } else {
            info!("Grade {} is out of range for image #{}", newgrade, image);
        }
    }
    info!("Missing image and/or angle to rotate or image not found");
    res.not_found("")
}

fn grade_params(req: &mut Request) -> QResult<(Option<i32>, Option<i16>)> {
    let data = req.form_body()?;
    Ok((
        data.get("image").and_then(|s| s.parse().ok()),
        data.get("grade").and_then(|s| s.parse().ok()),
    ))
}

pub fn set_location<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    if req.authorized_user().is_none() {
        return res.error(StatusCode::Unauthorized, "permission denied");
    }
    if let (Some(image), Some(coord)) = try_with!(res, location_params(req)) {
        info!("Should set location of #{} to {:?}.", image, coord);

        let (lat, lng) = ((coord.x * 1e6) as i32, (coord.y * 1e6) as i32);
        use diesel::insert_into;
        use schema::positions::dsl::*;
        let db: &PgConnection = &req.db_conn();
        insert_into(positions)
            .values((photo_id.eq(image), latitude.eq(lat), longitude.eq(lng)))
            .on_conflict(photo_id)
            .do_update()
            .set((latitude.eq(lat), longitude.eq(lng)))
            .execute(db)
            .expect("Insert image position");

        return res.redirect(format!("/img/{}", image));
    }
    info!("Missing image and/or position to set, or image not found.");
    res.not_found("")
}

fn location_params(
    req: &mut Request,
) -> QResult<(Option<i32>, Option<Coord>)> {
    let data = req.form_body()?;
    Ok((
        data.get("image").and_then(|s| s.parse().ok()),
        if let (Some(lat), Some(lng)) = (
            data.get("lat").and_then(|s| s.parse().ok()),
            data.get("lng").and_then(|s| s.parse().ok()),
        ) {
            Some(Coord { x: lat, y: lng })
        } else {
            None
        },
    ))
}

pub fn fetch_places<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    use diesel;
    if !req.authorized_user().is_some() {
        return res.error(StatusCode::Unauthorized, "permission denied");
    }
    let image = 60458;

    let c: &PgConnection = &req.db_conn();
    use schema::positions::dsl::*;
    let coord = match positions
        .filter(photo_id.eq(image))
        .select((latitude, longitude))
        .first::<(i32, i32)>(c)
    {
        Ok((tlat, tlong)) => Coord {
            x: f64::from(tlat) / 1e6,
            y: f64::from(tlong) / 1e6,
        },
        Err(diesel::NotFound) => {
            return res.not_found("Image has no position");
        }
        Err(err) => {
            error!("Failed to read position: {}", err);
            return res.not_found("Failed to get image position");
        }
    };
    info!("Should get places for {:?}", coord);
    let client = Client::new();
    match client
        .post("https://overpass.kumi.systems/api/interpreter")
        .body(format!(
            "[out:json];is_in({},{});area._[admin_level];out;",
            coord.x, coord.y,
        )).send()
    {
        Ok(mut response) => {
            if response.status().is_success() {
                let data = Json::from_reader(&mut response).unwrap();
                let obj = data.as_object().unwrap();
                if let Some(elements) =
                    obj.get("elements").and_then(|o| o.as_array())
                {
                    for obj in elements {
                        info!("{}", obj);
                        if let (Some(t_osm_id), Some((name, level))) =
                            (osm_id(obj), name_and_level(obj))
                        {
                            info!("{}: {} (level {})", t_osm_id, name, level);
                            let place_id = {
                                // http://overpass-api.de/api/interpreter?data=%5Bout%3Acustom%5D%3Brel%5Bref%3D%22A+555%22%5D%5Bnetwork%3DBAB%5D%3Bout%3B
                                use models::Place;
                                use schema::places::dsl::*;
                                places
                                    .filter(osm_id.eq(Some(t_osm_id)))
                                    .first::<Place>(c)
                                    .or_else(|_| {
                                        diesel::insert_into(places)
                                            .values((
                                                place_name.eq(&name),
                                                slug.eq(&slugify(&name)),
                                                osm_id.eq(Some(t_osm_id)),
                                                osm_level.eq(Some(level)),
                                            )).get_result::<Place>(c)
                                    }).expect("Find or create tag")
                            };
                            info!(" ...: {:?}", place_id)
                        }
                    }
                }
            } else {
                warn!("Bad response from overpass: {:?}", response);
            }
        }
        Err(err) => {
            warn!("Failed to get overpass info: {}", err);
        }
    }

    return res.ok(|o| writeln!(o, "Should get places for {:?}", coord));
}

fn osm_id(obj: &Json) -> Option<i64> {
    obj.find("id").and_then(|o| o.as_i64())
}

fn name_and_level(obj: &Json) -> Option<(&str, i16)> {
    obj.find("tags").and_then(|tags| {
        let name = tags
            .find("name:sv")
            //.or_else(|| tags.find("name:en"))
            .or_else(|| tags.find("name"))
            .and_then(|o| o.as_string());
        let level = tags
            .find("admin_level")
            .and_then(|o| o.as_string())
            .and_then(|s| s.parse().ok());
        if let (Some(name), Some(level)) = (name, level) {
            Some((name, level))
        } else {
            None
        }
    })
}
