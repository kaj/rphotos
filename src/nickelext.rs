//! A module of stuff that might evolve into improvements in nickel
//! itself.
//! Mainly, I'm experimenting with parsing url segments.

macro_rules! wrap2 {
    ($server:ident.$method:ident /:$slug:ident/, $handler:ident) => {
        $server.$method(
            concat!("/:", stringify!($slug), "/"),
            wrap!($handler: $slug)
                )
    };
    ($server:ident.$method:ident /:$slug1:ident/:$slug2:ident/, $handler:ident) => {
        $server.$method(
            concat!("/:", stringify!($slug1), "/:", stringify!($slug2), "/"),
            wrap!($handler: $slug1, $slug2)
                )
    };
    ($server:ident.$method:ident /:$slug1:ident/:$slug2:ident/:$slug3:ident/, $handler:ident) => {
        $server.$method(
            concat!("/:", stringify!($slug1), "/:", stringify!($slug2),
                    "/:", stringify!($slug3)),
            wrap!($handler: $slug1, $slug2, $slug3)
                )
    };
    ($server:ident.$method:ident /$path:ident, $handler:ident ) => {
        $server.$method(concat!("/", stringify!($path)), $handler)
    };
    ($server:ident.$method:ident /$path:ident/, $handler:ident ) => {
        $server.$method(concat!("/", stringify!($path), "/"), $handler)
    };
    ($server:ident.$method:ident /$path:ident/:$slug:ident, $handler:ident) => {
        $server.$method(
            concat!("/", stringify!($path), "/:", stringify!($slug)),
            wrap!($handler: $slug)
                )
    };
    ($server:ident.$method:ident /$path:ident/:$slug:ident/:$slug2:ident, $handler:ident) => {
        $server.$method(
            concat!("/", stringify!($path), "/:", stringify!($slug),
                    "/:", stringify!($slug2)),
            wrap!($handler: $slug, $slug2)
                )
    };
}

macro_rules! wrap {
    ($handler:ident : $( $param:ident ),+ ) => { {
        #[allow(unused_parens)]
        fn wrapped<'mw>(req: &mut Request,
                        res: Response<'mw>)
                        -> MiddlewareResult<'mw> {
            if let ($(Some($param),)*) = ($(opt(req, stringify!($param)),)*) {
                $handler(req, res, $($param),*)
            } else {
                res.error(StatusCode::NotFound, "Parameter mismatch")
            }
        }
        wrapped
    } }
}

pub trait FromSlug : Sized {
    fn parse_slug(slug: &str) -> Option<Self>;
}
impl FromSlug for String {
    fn parse_slug(slug: &str) -> Option<Self> {
        Some(slug.to_string())
    }
}
impl FromSlug for i32 {
    fn parse_slug(slug: &str) -> Option<Self> {
        slug.parse::<Self>().ok()
    }
}
impl FromSlug for u8 {
    fn parse_slug(slug: &str) -> Option<Self> {
        slug.parse::<Self>().ok()
    }
}
impl FromSlug for u16 {
    fn parse_slug(slug: &str) -> Option<Self> {
        slug.parse::<Self>().ok()
    }
}
impl FromSlug for u32 {
    fn parse_slug(slug: &str) -> Option<Self> {
        slug.parse::<Self>().ok()
    }
}

