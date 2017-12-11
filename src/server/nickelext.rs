//! A module of stuff that might evolve into improvements in nickel
//! itself.
//! Mainly, I'm experimenting with parsing url segments.
use hyper::header::{Expires, HttpDate};
use nickel::{Halt, MiddlewareResult, Response};
use nickel::status::StatusCode;
use std::io::{self, Write};
use time::{now, Duration};

macro_rules! wrap3 {
    ($server:ident.$method:ident $url:expr,
     $handler:ident : $( $param:ident ),*) => {{
         #[allow(unused_parens)]
         fn wrapped<'mw>(req: &mut Request,
                         res: Response<'mw>)
                         -> MiddlewareResult<'mw> {
             if let ($(Some($param),)*) =
                 ($(req.param(stringify!($param)).and_then(FromSlug::parse),)*)
             {
                 $handler(req, res, $($param),*)
             } else {
                 res.not_found("Parameter mismatch")
             }
         }
         let matcher = format!($url, $(concat!(":", stringify!($param))),+);
         info!("Route {} {} to {}",
               stringify!($method),
               matcher,
               stringify!($handler));
         $server.$method(matcher, wrapped);
     }};
    ($server:ident.$method:ident $url:expr, $handler:ident) => {
        info!("Route {} {} to {}",
              stringify!($method),
              $url,
              stringify!($handler));
        $server.$method($url, $handler);
    };
}

pub trait FromSlug: Sized {
    fn parse(slug: &str) -> Option<Self>;
}
impl FromSlug for String {
    fn parse(slug: &str) -> Option<Self> {
        Some(slug.to_string())
    }
}
impl FromSlug for i32 {
    fn parse(slug: &str) -> Option<Self> {
        slug.parse::<Self>().ok()
    }
}
impl FromSlug for u8 {
    fn parse(slug: &str) -> Option<Self> {
        slug.parse::<Self>().ok()
    }
}
impl FromSlug for u16 {
    fn parse(slug: &str) -> Option<Self> {
        slug.parse::<Self>().ok()
    }
}
impl FromSlug for u32 {
    fn parse(slug: &str) -> Option<Self> {
        slug.parse::<Self>().ok()
    }
}
impl FromSlug for usize {
    fn parse(slug: &str) -> Option<Self> {
        slug.parse::<Self>().ok()
    }
}

pub trait MyResponse<'mw> {
    fn ok<F>(self, do_render: F) -> MiddlewareResult<'mw>
    where
        F: FnOnce(&mut Write) -> io::Result<()>;

    fn not_found(self, msg: &'static str) -> MiddlewareResult<'mw>;
}

impl<'mw> MyResponse<'mw> for Response<'mw> {
    fn ok<F>(self, do_render: F) -> MiddlewareResult<'mw>
    where
        F: FnOnce(&mut Write) -> io::Result<()>,
    {
        let mut stream = self.start()?;
        match do_render(&mut stream) {
            Ok(()) => Ok(Halt(stream)),
            Err(e) => {
                stream.bail(format!("Error rendering template: {:?}", e))
            }
        }
    }

    fn not_found(self, msg: &'static str) -> MiddlewareResult<'mw> {
        self.error(StatusCode::NotFound, msg)
    }
}

pub fn far_expires() -> Expires {
    Expires(HttpDate(now() + Duration::days(300)))
}
