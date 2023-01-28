use super::Context;
use crate::photosdir::ImageLoadFailed;
use crate::templates::{self, RenderError, RenderRucte};
use tracing::{error, warn};
use warp::http::response::Builder;
use warp::http::status::StatusCode;
use warp::reply::Response;
use warp::{self, Rejection, Reply};

pub enum ViewError {
    /// 404
    NotFound(Option<Context>),
    /// 400
    BadRequest(&'static str),
    PermissionDenied,
    /// 503
    ServiceUnavailable,
    /// 500
    Err(&'static str),
}

macro_rules! or_404 {
    ($obj:expr, $ctx:expr) => {{
        match $obj {
            Some(obj) => obj,
            None => return Err(ViewError::NotFound(Some($ctx))),
        }
    }};
    ($obj:expr) => {{
        match $obj {
            Some(obj) => obj,
            None => return Err(ViewError::NotFound(None)),
        }
    }};
}
macro_rules! or_404q {
    ($obj:expr, $ctx:expr) => {
        or_404!($obj.optional()?, $ctx)
    };
}

pub trait ViewResult<T> {
    fn ise(self) -> Result<T, ViewError>;
    fn req(self, msg: &'static str) -> Result<T, ViewError>;
}

impl<T, E> ViewResult<T> for Result<T, E>
where
    E: std::fmt::Debug,
{
    fn ise(self) -> Result<T, ViewError> {
        self.map_err(|e| {
            error!("Internal server error: {:?}", e);
            ViewError::Err("Something went wrong")
        })
    }
    fn req(self, msg: &'static str) -> Result<T, ViewError> {
        self.map_err(|e| {
            warn!("Bad request, {}: {:?}", msg, e);
            ViewError::BadRequest(msg)
        })
    }
}

impl Reply for ViewError {
    fn into_response(self) -> Response {
        match self {
            ViewError::NotFound(Some(context)) => Builder::new()
                .status(StatusCode::NOT_FOUND)
                .html(|o| {
                    templates::not_found_html(
                        o,
                        &context,
                        StatusCode::NOT_FOUND,
                        "The resource you requested could not be located",
                    )
                })
                .unwrap_or_else(|_| StatusCode::NOT_FOUND.into_response()),
            ViewError::NotFound(None) => error_response(
                StatusCode::NOT_FOUND,
                "Not found",
                "The resource you requested could not be located.",
            ),
            ViewError::BadRequest(msg) => {
                error_response(StatusCode::BAD_REQUEST, msg, "Sorry.")
            }
            ViewError::PermissionDenied => {
                error_response(StatusCode::UNAUTHORIZED, "Sorry", "Sorry.")
            }
            ViewError::ServiceUnavailable => error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Server exhausted",
                "The server is exhausted and can't handle your request \
                 right now. Sorry. \
                 Please try again later.",
            ),
            ViewError::Err(msg) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                msg,
                "This is an error in the server code or configuration. \
                 Sorry. \
                 The error has been logged and I will try to fix it.",
            ),
        }
    }
}

fn error_response(code: StatusCode, message: &str, detail: &str) -> Response {
    Builder::new()
        .status(code)
        .html(|o| templates::error_html(o, code, message, detail))
        .unwrap_or_else(|_| code.into_response())
}

impl From<RenderError> for ViewError {
    fn from(e: RenderError) -> Self {
        error!("Rendering error: {}\n    {:?}", e, e);
        ViewError::Err("Rendering error")
    }
}

impl From<diesel::result::Error> for ViewError {
    fn from(e: diesel::result::Error) -> Self {
        error!("Database error: {}\n    {:?}", e, e);
        ViewError::Err("Database error")
    }
}

impl From<r2d2_memcache::memcache::Error> for ViewError {
    fn from(e: r2d2_memcache::memcache::Error) -> Self {
        error!("Pool error: {:?}", e);
        ViewError::Err("Pool error")
    }
}
impl From<ImageLoadFailed> for ViewError {
    fn from(e: ImageLoadFailed) -> Self {
        error!("Image load error: {:?}", e);
        ViewError::Err("Failed to load image")
    }
}

/// Create custom errors for warp rejections.
///
/// Currently only handles 404, as there is no way of getting any
/// details out of the other build-in rejections in warp.
pub async fn for_rejection(err: Rejection) -> Result<Response, Rejection> {
    if err.is_not_found() {
        Ok(ViewError::NotFound(None).into_response())
    } else {
        Err(err)
    }
}
