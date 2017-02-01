//! A module of stuff that might evolve into improvements in nickel
//! itself.
//! Mainly, I'm experimenting with parsing url segments.

macro_rules! wrap3 {
    ($server:ident.$method:ident $url:expr,
     $handler:ident : $( $param:ident ),*) => {{
         #[allow(unused_parens)]
         fn wrapped<'mw>(req: &mut Request,
                         res: Response<'mw>)
                         -> MiddlewareResult<'mw> {
             if let ($(Some($param),)*) =
                 ($(req.param(stringify!($param)).and_then(FromSlug::parse),)*) {
                     $handler(req, res, $($param),*)
                 } else {
                     res.error(StatusCode::NotFound, "Parameter mismatch")
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
