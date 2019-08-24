/// This module defines the `RenderRucte` trait for a response builer.
///
/// If ructe gets a warp feature, this is probably it.
use chrono::{Duration, Utc};
use mime::TEXT_HTML_UTF_8;
use std::io::{self, Write};
use warp::http::response::Builder;
use warp::http::{header, Response, StatusCode};

pub trait RenderRucte {
    fn html<F>(&mut self, f: F) -> Response<Vec<u8>>
    where
        F: FnOnce(&mut dyn Write) -> io::Result<()>;

    fn redirect(&mut self, url: &str) -> Response<Vec<u8>>;

    fn far_expires(&mut self) -> &mut Self;
}

impl RenderRucte for Builder {
    fn html<F>(&mut self, f: F) -> Response<Vec<u8>>
    where
        F: FnOnce(&mut dyn Write) -> io::Result<()>,
    {
        let mut buf = Vec::new();
        f(&mut buf).unwrap();
        self.header("content-type", TEXT_HTML_UTF_8.as_ref())
            .body(buf)
            .unwrap()
    }
    fn redirect(&mut self, url: &str) -> Response<Vec<u8>> {
        self.status(StatusCode::FOUND)
            .header(header::LOCATION, url)
            .body(format!("Please refer to {}", url).into_bytes())
            .unwrap()
    }

    fn far_expires(&mut self) -> &mut Self {
        let far_expires = Utc::now() + Duration::days(180);
        self.header(header::EXPIRES, far_expires.to_rfc2822())
    }
}
