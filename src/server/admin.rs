//! Admin-only views, generally called by javascript.
use super::SizeTag;
use diesel::prelude::*;
use memcachemiddleware::MemcacheRequestExtensions;
use models::Photo;
use nickel::{BodyError, FormBody, MiddlewareResult, Request, Response};
use nickel::extensions::Redirect;
use nickel::status::StatusCode;
use nickel_diesel::DieselRequestExtensions;
use nickel_jwt_session::SessionRequestExtensions;
use server::nickelext::MyResponse;

pub fn rotate<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    if !req.authorized_user().is_some() {
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
                    req.clear_cache(&image.cache_key(&SizeTag::Small));
                    req.clear_cache(&image.cache_key(&SizeTag::Medium));
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
    if !req.authorized_user().is_some() {
        return res.error(StatusCode::Unauthorized, "permission denied");
    }
    if let (Some(image), Some(tag)) = try_with!(res, tag_params(req)) {
        let c: &PgConnection = &req.db_conn();
        use diesel;
        use models::{NewPhotoTag, NewTag, PhotoTag, Tag};
        let tag = {
            use schema::tags::dsl::*;
            tags.filter(tag_name.ilike(&tag))
                .first::<Tag>(c)
                .or_else(|_| {
                    diesel::insert(
                        &NewTag { tag_name: &tag, slug: &slugify(&tag) },
                    ).into(tags)
                        .get_result::<Tag>(c)
                })
                .expect("Find or create tag")
        };
        use schema::photo_tags::dsl::*;
        let q = photo_tags
            .filter(photo_id.eq(image))
            .filter(tag_id.eq(tag.id));
        if q.first::<PhotoTag>(c).is_ok() {
            info!("Photo #{} already has {:?}", image, tag);
        } else {
            info!("Add {:?} on photo #{}!", tag, image);
            diesel::insert(&NewPhotoTag { photo_id: image, tag_id: tag.id })
                .into(photo_tags)
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
    if !req.authorized_user().is_some() {
        return res.error(StatusCode::Unauthorized, "permission denied");
    }
    if let (Some(image), Some(name)) = try_with!(res, person_params(req)) {
        let c: &PgConnection = &req.db_conn();
        use diesel;
        use models::{NewPerson, NewPhotoPerson, Person, PhotoPerson};
        let person = {
            use schema::people::dsl::*;
            people
                .filter(person_name.ilike(&name))
                .first::<Person>(c)
                .or_else(|_| {
                    diesel::insert(&NewPerson {
                        person_name: &name,
                        slug: &slugify(&name),
                    }).into(people)
                        .get_result::<Person>(c)
                })
                .expect("Find or create tag")
        };
        use schema::photo_people::dsl::*;
        let q = photo_people
            .filter(photo_id.eq(image))
            .filter(person_id.eq(person.id));
        if q.first::<PhotoPerson>(c).is_ok() {
            info!("Photo #{} already has {:?}", image, person);
        } else {
            info!("Add {:?} on photo #{}!", person, image);
            diesel::insert(
                &NewPhotoPerson { photo_id: image, person_id: person.id },
            ).into(photo_people)
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

pub fn slugify(val: &str) -> String {
    val.chars()
        .map(|c| match c {
            c @ '0'...'9' | c @ 'a'...'z' => c,
            c @ 'A'...'Z' => (c as u8 - b'A' + b'a') as char,
            'Å' | 'å' | 'Ä' | 'ä' | 'Â' | 'â' => 'a',
            'Ö' | 'ö' | 'Ô' | 'ô' => 'o',
            'É' | 'é' | 'Ë' | 'ë' | 'Ê' | 'ê' => 'e',
            'Ü' | 'ü' | 'Û' | 'û' => 'u',
            _ => '_',
        })
        .collect()
}

pub fn set_grade<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    if !req.authorized_user().is_some() {
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
