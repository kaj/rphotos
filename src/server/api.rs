//! API views
use super::Context;
use crate::models::{Photo, SizeTag};
use diesel::{self, prelude::*, result::Error as DbError, update};
use log::warn;
use serde::{Deserialize, Serialize};
use warp::filters::BoxedFilter;
use warp::http::StatusCode;
use warp::reply::Response;
use warp::{Filter, Reply};

pub fn routes(s: BoxedFilter<(Context,)>) -> BoxedFilter<(impl Reply,)> {
    use warp::filters::method::v2::{get, post};
    use warp::path::{end, path};
    use warp::{body, query};
    let login = path("login").and(end()).and(post()).and(s.clone())
        .and(body::json()).map(login);
    let gimg = end().and(get()).and(s.clone()).and(query()).map(get_img);
    let pimg = path("makepublic")
        .and(end())
        .and(post())
        .and(s.clone())
        .and(body::json())
        .map(make_public);

    login.or(path("image").and(gimg.or(pimg))).boxed()
}
use super::login::LoginForm;

fn login(context: Context, form: LoginForm) -> Response {
    context.db()
        .map_err(Into::into)
        .and_then(|db| {
            let user = form.validate(&db)
                .ok_or_else(|| ApiError::bad_request("login failed"))?;
            Ok(
                warp::reply::json(&LoginOk {
                    token: context.make_token(&user)
                        .ok_or_else(|| ApiError::bad_request("failed to make token"))?,
                })
                .into_response()
            )
        })
        .unwrap_or_else(|err: ApiError| err.into_response())
}

#[derive(Debug, Serialize)]
struct LoginOk {
    token: String
}

#[derive(Debug, Deserialize)]
struct ImgQuery {
    id: Option<u32>,
    path: Option<String>,
}

impl ImgQuery {
    fn validate(self) -> Result<ImgIdentifier, &'static str> {
        match (self.id, self.path) {
            (None, None) => Err("id or path required"),
            (Some(id), None) => Ok(ImgIdentifier::Id(id)),
            (None, Some(path)) => Ok(ImgIdentifier::Path(path)),
            (Some(_), Some(_)) => Err("Conflicting arguments"),
        }
    }
}

enum ImgIdentifier {
    Id(u32),
    Path(String),
}

impl ImgIdentifier {
    fn load(&self, db: &PgConnection) -> Result<Option<Photo>, DbError> {
        use crate::schema::photos::dsl as p;
        match &self {
            ImgIdentifier::Id(ref id) => {
                p::photos.filter(p::id.eq(*id as i32)).first(db)
            }
            ImgIdentifier::Path(path) => {
                p::photos.filter(p::path.eq(path)).first(db)
            }
        }
        .optional()
    }
}

fn get_img(context: Context, q: ImgQuery) -> Response {
    q.validate()
        .map_err(ApiError::bad_request)
        .and_then(|id| {
            let db = context.db()?;
            let img = id.load(&db)?.ok_or(NOT_FOUND)?;
            if !context.is_authorized() && !img.is_public() {
                return Err(NOT_FOUND);
            }
            Ok(
                warp::reply::json(&GetImgResult::for_img(&img))
                    .into_response(),
            )
        })
        .unwrap_or_else(|err| err.into_response())
}

fn make_public(context: Context, q: ImgQuery) -> Response {
    if !context.is_authorized() {
        return ApiError {
            code: StatusCode::UNAUTHORIZED,
            msg: "Authorization required",
        }
        .into_response();
    }
    q.validate()
        .map_err(ApiError::bad_request)
        .and_then(|id| {
            let db = context.db()?;
            let img = id.load(&db)?.ok_or(NOT_FOUND)?;
            use crate::schema::photos::dsl as p;
            let img = update(p::photos.find(img.id))
                .set(p::is_public.eq(true))
                .get_result(&db)?;
            Ok(
                warp::reply::json(&GetImgResult::for_img(&img))
                    .into_response(),
            )
        })
        .unwrap_or_else(|err| err.into_response())
}

struct ApiError {
    code: StatusCode,
    msg: &'static str,
}

const NOT_FOUND: ApiError = ApiError::bad_request("not found");

impl ApiError {
    const fn bad_request(msg: &'static str) -> Self {
        ApiError {
            code: StatusCode::BAD_REQUEST,
            msg,
        }
    }
    fn into_response(self) -> Response {
        let mut response =
            warp::reply::json(&ApiErrorMessage { err: self.msg })
                .into_response();
        *response.status_mut() = self.code;
        response
    }
}

impl From<diesel::result::Error> for ApiError {
    fn from(err: diesel::result::Error) -> ApiError {
        warn!("Diesel error in api: {}", err);
        ApiError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            msg: "database error",
        }
    }
}
impl From<r2d2_memcache::r2d2::Error> for ApiError {
    fn from(err: r2d2_memcache::r2d2::Error) -> ApiError {
        warn!("R2D2 error in api: {}", err);
        ApiError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            msg: "pool error",
        }
    }
}

#[derive(Debug, Serialize)]
struct ApiErrorMessage {
    err: &'static str,
}

#[derive(Debug, Serialize)]
struct GetImgResult {
    small: ImgLink,
    medium: ImgLink,
    public: bool,
}

impl GetImgResult {
    fn for_img(img: &Photo) -> Self {
        GetImgResult {
            small: ImgLink::new(img, SizeTag::Small),
            medium: ImgLink::new(img, SizeTag::Medium),
            public: img.is_public,
        }
    }
}

#[derive(Debug, Serialize)]
struct ImgLink {
    url: String,
    width: u32,
    height: u32,
}

impl ImgLink {
    fn new(img: &Photo, size: SizeTag) -> Self {
        let (width, height) = img.get_size(size);
        ImgLink {
            url: format!("/img/{}-{}.jpg", img.id, size.tag()),
            width,
            height,
        }
    }
}
