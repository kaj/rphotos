//! API views
use super::login::LoginForm;
use super::{Context, ViewError};
use crate::models::{Photo, SizeTag};
use diesel::{self, prelude::*, result::Error as DbError, update};
use serde::{Deserialize, Serialize};
use warp::filters::BoxedFilter;
use warp::http::StatusCode;
use warp::reject::MethodNotAllowed;
use warp::reply::Response;
use warp::{Filter, Rejection, Reply};

type ApiResult<T> = Result<T, ApiError>;

pub fn routes(s: BoxedFilter<(Context,)>) -> BoxedFilter<(impl Reply,)> {
    use warp::filters::method::{get, post};
    use warp::path::{end, path};
    use warp::{body, query};
    let login = path("login")
        .and(end())
        .and(post())
        .and(s.clone())
        .and(body::json())
        .map(login)
        .map(w);
    let gimg = end().and(get()).and(s.clone()).and(query()).map(get_img);
    let pimg = path("makepublic")
        .and(end())
        .and(post())
        .and(s)
        .and(body::json())
        .map(make_public);

    login
        .or(path("image").and(gimg.or(pimg).unify().map(w)))
        .recover(api_recover)
        .boxed()
}

async fn api_recover(err: Rejection) -> Result<Response, Rejection> {
    let code = if err.is_not_found() {
        StatusCode::NOT_FOUND
    } else if err.find::<MethodNotAllowed>().is_some() {
        StatusCode::METHOD_NOT_ALLOWED
    } else {
        log::error!("Internal server error in api from {err:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    };
    let msg = code.canonical_reason().unwrap_or("error");
    Ok(ApiError { code, msg }.into_response())
}

fn w<T: Serialize>(result: ApiResult<T>) -> Response {
    match result {
        Ok(result) => warp::reply::json(&result).into_response(),
        Err(err) => err.into_response(),
    }
}

fn login(context: Context, form: LoginForm) -> ApiResult<LoginOk> {
    let db = context.db()?;
    let user = form
        .validate(&db)
        .ok_or_else(|| ApiError::bad_request("login failed"))?;
    log::info!("Api login {user:?} ok");
    Ok(LoginOk {
        token: context.make_token(&user)?,
    })
}

#[derive(Debug, Serialize)]
struct LoginOk {
    token: String,
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

fn get_img(context: Context, q: ImgQuery) -> ApiResult<GetImgResult> {
    let id = q.validate().map_err(ApiError::bad_request)?;
    let db = context.db()?;
    let img = id.load(&db)?.ok_or(NOT_FOUND)?;
    if !context.is_authorized() && !img.is_public() {
        return Err(NOT_FOUND);
    }
    Ok(GetImgResult::for_img(&img))
}

fn make_public(context: Context, q: ImgQuery) -> ApiResult<GetImgResult> {
    if !context.is_authorized() {
        return Err(ApiError {
            code: StatusCode::UNAUTHORIZED,
            msg: "Authorization required",
        });
    }
    let id = q.validate().map_err(ApiError::bad_request)?;
    let db = context.db()?;
    let img = id.load(&db)?.ok_or(NOT_FOUND)?;
    use crate::schema::photos::dsl as p;
    let img = update(p::photos.find(img.id))
        .set(p::is_public.eq(true))
        .get_result(&db)?;
    Ok(GetImgResult::for_img(&img))
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

impl<T> From<T> for ApiError
where
    ViewError: From<T>,
{
    fn from(err: T) -> ApiError {
        match ViewError::from(err) {
            ViewError::NotFound(_) => ApiError {
                code: StatusCode::NOT_FOUND,
                msg: "Not found",
            },
            ViewError::BadRequest(msg) => ApiError::bad_request(msg),
            ViewError::PermissionDenied => ApiError {
                code: StatusCode::UNAUTHORIZED,
                msg: "Permission denied",
            },
            ViewError::ServiceUnavailable => ApiError {
                code: StatusCode::SERVICE_UNAVAILABLE,
                msg: "Server exhausted",
            },
            ViewError::Err(msg) => ApiError {
                code: StatusCode::INTERNAL_SERVER_ERROR,
                msg,
            },
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
