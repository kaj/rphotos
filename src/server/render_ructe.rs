use chrono::{Duration, Utc};
use warp::http::response::Builder;
use warp::http::{header, StatusCode};
use warp::reply::Response;

pub trait BuilderExt {
    fn redirect(self, url: &str) -> Response;

    fn far_expires(self) -> Self;
}

impl BuilderExt for Builder {
    fn redirect(self, url: &str) -> Response {
        self.status(StatusCode::FOUND)
            .header(header::LOCATION, url)
            .body(format!("Please refer to {url}").into())
            .unwrap()
    }

    fn far_expires(self) -> Self {
        let far_expires = Utc::now() + Duration::days(180);
        self.header(header::EXPIRES, far_expires.to_rfc2822())
    }
}
