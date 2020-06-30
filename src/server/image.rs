use super::BuilderExt;
use super::{error_response, not_found, Context};
use crate::models::{Photo, SizeTag};
use crate::photosdir::{get_scaled_jpeg, ImageLoadFailed};
use diesel::prelude::*;
use std::str::FromStr;
use warp::http::response::Builder;
use warp::http::{header, StatusCode};
use warp::reply::Response;
use warp::Rejection;

pub async fn show_image(
    img: ImgName,
    context: Context,
) -> Result<Response, Rejection> {
    use crate::schema::photos::dsl::photos;
    let tphoto = photos.find(img.id).first::<Photo>(&context.db().unwrap());
    if let Ok(tphoto) = tphoto {
        if context.is_authorized() || tphoto.is_public() {
            if img.size == SizeTag::Large {
                if context.is_authorized() {
                    use std::fs::File;
                    use std::io::Read;
                    // TODO: This should be done in a more async-friendly way.
                    let path = context.photos().get_raw_path(&tphoto);
                    let mut buf = Vec::new();
                    if File::open(path)
                        .map(|mut f| f.read_to_end(&mut buf))
                        .is_ok()
                    {
                        return Ok(Builder::new()
                            .status(StatusCode::OK)
                            .header(
                                header::CONTENT_TYPE,
                                mime::IMAGE_JPEG.as_ref(),
                            )
                            .far_expires()
                            .body(buf.into())
                            .unwrap());
                    } else {
                        return Ok(error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                        )
                        .unwrap());
                    }
                }
            } else {
                let data = get_image_data(&context, &tphoto, img.size)
                    .await
                    .expect("Get image data");
                return Ok(Builder::new()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, mime::IMAGE_JPEG.as_ref())
                    .far_expires()
                    .body(data.into())
                    .unwrap());
            }
        }
    }
    Ok(not_found(&context))
}

/// A client-side / url file name for a file.
/// Someting like 4711-s.jpg
#[derive(Debug, Eq, PartialEq)]
pub struct ImgName {
    id: i32,
    size: SizeTag,
}

#[derive(Debug, Eq, PartialEq)]
pub struct BadImgName {}

impl FromStr for ImgName {
    type Err = BadImgName;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(pos) = s.find('-') {
            let (num, rest) = s.split_at(pos);
            let id = num.parse().map_err(|_| BadImgName {})?;
            let size = match rest {
                "-s.jpg" => SizeTag::Small,
                "-m.jpg" => SizeTag::Medium,
                "-l.jpg" => SizeTag::Large,
                _ => return Err(BadImgName {}),
            };
            return Ok(ImgName { id, size });
        }
        Err(BadImgName {})
    }
}

#[test]
fn parse_good_imgname() {
    assert_eq!(
        "4711-s.jpg".parse(),
        Ok(ImgName {
            id: 4711,
            size: SizeTag::Small,
        })
    )
}

#[test]
fn parse_bad_imgname_1() {
    assert_eq!("4711-q.jpg".parse::<ImgName>(), Err(BadImgName {}))
}
#[test]
fn parse_bad_imgname_2() {
    assert_eq!("blurgel".parse::<ImgName>(), Err(BadImgName {}))
}

async fn get_image_data(
    context: &Context,
    photo: &Photo,
    size: SizeTag,
) -> Result<Vec<u8>, ImageLoadFailed> {
    let p = context.photos().get_raw_path(photo);
    let r = photo.rotation;
    context
        .cached_or(&photo.cache_key(size), || get_scaled_jpeg(p, r, size.px()))
        .await
}
