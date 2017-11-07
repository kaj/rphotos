//! Admin-only views, generally called by javascript.
use nickel::{BodyError, FormBody, MiddlewareResult, Request, Response};
use nickel::extensions::Redirect;
use nickel::status::StatusCode;
use nickel_diesel::DieselRequestExtensions;
use nickel_jwt_session::SessionRequestExtensions;
use server::nickelext::MyResponse;
use models::Photo;
use diesel::prelude::*;
use super::SizeTag;
use memcachemiddleware::MemcacheRequestExtensions;

pub fn rotate<'mw>(req: &mut Request,
                   res: Response<'mw>)
                   -> MiddlewareResult<'mw> {
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
                    return res.ok(|o| writeln!(o, "ok"))
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

pub fn rotate_params(req: &mut Request)
                     -> Result<(Option<i32>, Option<i16>),
                               (StatusCode, BodyError)>
{
    let form_data = req.form_body()?;
    Ok((form_data.get("image").and_then(|s| s.parse().ok()),
        form_data.get("angle").and_then(|s| s.parse().ok())))
}

pub fn tag<'mw>(req: &mut Request,
                   res: Response<'mw>)
                   -> MiddlewareResult<'mw> {
    if !req.authorized_user().is_some() {
        return res.error(StatusCode::Unauthorized, "permission denied");
    }
    if let (Some(image), Some(tag)) = try_with!(res, tag_params(req)) {
        let c: &PgConnection = &req.db_conn();
        use models::{NewPhotoTag, NewTag, PhotoTag, Tag};
        use diesel;
        let tag = {
            use schema::tags::dsl::*;
            tags.filter(tag_name.ilike(&tag))
                .first::<Tag>(c)
                .or_else(|_| {
                             diesel::insert(&NewTag {
                                 tag_name: &tag,
                                 slug: &slugify(&tag),
                         })
                        .into(tags)
                        .get_result::<Tag>(c)
                })
                .expect("Find or create tag")
        };
        use schema::photo_tags::dsl::*;
        let q = photo_tags.filter(photo_id.eq(image))
            .filter(tag_id.eq(tag.id));
        if q.first::<PhotoTag>(c).is_ok() {
            info!("Photo #{} already has {:?}", image, tag);
        } else {
            info!("Add {:?} on photo #{}!", tag, image);
            diesel::insert(&NewPhotoTag {
                    photo_id: image,
                    tag_id: tag.id,
                })
                .into(photo_tags)
                .execute(c)
                .expect("Tag a photo");
        }
        return res.redirect(format!("/img/{}", image));
    }
    info!("Missing image and/or angle to rotate or image not found");
    res.not_found("")
}

pub fn tag_params(req: &mut Request)
                     -> Result<(Option<i32>, Option<String>),
                               (StatusCode, BodyError)>
{
    let form_data = req.form_body()?;
    Ok((form_data.get("image").and_then(|s| s.parse().ok()),
        form_data.get("tag").map(String::from)))
}

pub fn slugify(val: &str) -> String {
    val.chars()
        .map(|c| match c {
            c @ '0'...'9' | c @ 'a'...'z'=> c,
            c @ 'A'...'Z' => (c as u8 - b'A' + b'a') as char,
            'Å' | 'å' | 'Ä' | 'ä' | 'Â' | 'â' => 'a',
            'Ö' | 'ö' | 'Ô' | 'ô' => 'o',
            'É' | 'é' | 'Ë' | 'ë' | 'Ê' | 'ê' => 'e',
            'Ü' | 'ü' | 'Û' | 'û' => 'u',
            _ => '_',
        })
        .collect()
}
