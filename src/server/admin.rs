//! Admin-only views, generally called by javascript.
use nickel::{BodyError, FormBody, MiddlewareResult, Request, Response};
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
