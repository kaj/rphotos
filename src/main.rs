#[macro_use] extern crate nickel;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate nickel_jwt_session;
extern crate rustorm;
extern crate rustc_serialize;
extern crate typemap;
extern crate plugin;
extern crate image;
extern crate hyper;
extern crate time;
extern crate chrono;
extern crate rexif;

use chrono::UTC;
use chrono::offset::TimeZone;
use chrono::{Duration as ChDuration};
use chrono::Datelike;
use hyper::header::{Expires, HttpDate};
use nickel::{MediaType, HttpRouter, Nickel, StaticFilesHandler, Request, Response, MiddlewareResult};
use nickel::extensions::response::Redirect;
use nickel_jwt_session::{SessionMiddleware, SessionRequestExtensions, SessionResponseExtensions};
use plugin::{Pluggable};
use rustc_serialize::Encodable;
use rustorm::query::{Query, Filter};
use time::Duration;
use nickel::status::StatusCode;
use std::io::Read;

mod models;
use models::{Entity, Photo, Tag, Person, Place, query_for};

mod env;
use env::{dburl, env_or, jwt_key, photos_dir};

mod photosdir;

mod rustormmiddleware;
use rustormmiddleware::{RustormMiddleware, RustormRequestExtensions};

mod requestloggermiddleware;
use requestloggermiddleware::RequestLoggerMiddleware;

mod photosdirmiddleware;
use photosdirmiddleware::{PhotosDirMiddleware, PhotosDirRequestExtensions};


macro_rules! render {
    ($res:expr, $template:expr, { $($param:ident : $ptype:ty = $value:expr),* })
        =>
    {
        {
        #[derive(Debug, Clone, RustcEncodable)]
        struct ParamData {
            csslink: String,
            $(
                $param: $ptype,
                )*
        }
        $res.render($template, &ParamData {
            csslink: include!(concat!(env!("OUT_DIR"), "/stylelink")).into(),
            $(
                $param: $value,
                )*
        })
        }
    }
}

fn orm_get_related<T: Entity, Src: Entity>(src: &Src, rel_table: &str)
                                           -> Query
{
    let mut q = Query::select();
    q.only_from(&T::table());
    q.left_join_table(rel_table, &format!("{}.id", T::table().name),
                      &format!("{}.{}", rel_table, T::table().name))
        .filter_eq(&format!("{}.{}", rel_table, Src::table().name), src.id());
    q
}

#[derive(Debug, Clone, RustcEncodable)]
struct Group {
    title: String,
    url: String,
    count: i64,
    photo: Photo,
}

fn monthname(n: u8) -> &'static str {
    match n {
        1 => "january",
        2 => "february",
        3 => "march",
        4 => "april",
        5 => "may",
        6 => "june",
        7 => "july",
        8 => "august",
        9 => "september",
        10 => "october",
        11 => "november",
        12 => "december",
        _ => "non-month"
    }
}

fn main() {
    env_logger::init().unwrap();
    info!("Initalized logger");

    let mut server = Nickel::new();
    server.utilize(RequestLoggerMiddleware);
    server.utilize(SessionMiddleware::new(&jwt_key()));
    // TODO This is a "build" location, not an "install" location ...
    let staticdir = concat!(env!("OUT_DIR"), "/static/");
    info!("Serving static files from {}", staticdir);
    server.utilize(StaticFilesHandler::new(staticdir));
    server.utilize(RustormMiddleware::new(&dburl()));
    server.utilize(PhotosDirMiddleware::new(photos_dir()));

    server.get("/login",             login);
    server.post("/login",            do_login);
    server.get("/logout",            logout);
    server.get("/",                  all_years);
    server.get("/img/:id/:size",     show_image);
    server.get("/tag/",              tag_all);
    server.get("/tag/:tag",          tag_one);
    server.get("/place/",            place_all);
    server.get("/place/:slug",       place_one);
    server.get("/person/",           person_all);
    server.get("/person/:slug",      person_one);
    server.get("/details/:id",       photo_details);
    server.get("/:year/",            months_in_year);
    server.get("/:year/:month/",     days_in_month);
    server.get("/:year/:month/:day", all_for_day);

    server.listen(&*env_or("RPHOTOS_LISTEN", "127.0.0.1:6767"));
}

fn login<'mw>(_req: &mut Request, mut res: Response<'mw>)
              -> MiddlewareResult<'mw> {
    res.clear_jwt_user();
    render!(res, "templates/login.tpl", {})
}

fn do_login<'mw>(req: &mut Request, mut res: Response<'mw>)
                 -> MiddlewareResult<'mw> {
    // TODO It seems form data parsing is next version of nickel ...
    let mut form_data = String::new();
    req.origin.read_to_string(&mut form_data).unwrap();
    println!("Form: {:?}", form_data);
    if form_data == "user=kaj&password=kaj123" {
        res.set_jwt_user("kaj");
        res.redirect("/")
    } else {
        render!(res, "templates/login.tpl", {})
    }
}

fn logout<'mw>(_req: &mut Request, mut res: Response<'mw>)
               -> MiddlewareResult<'mw>  {
    res.clear_jwt_user();
    res.redirect("/")
}

fn show_image<'mw>(req: &mut Request, mut res: Response<'mw>)
              -> MiddlewareResult<'mw>  {
    if let Ok(id) = req.param("id").unwrap().parse::<i32>() {
        if let Ok(photo) = req.orm_get::<Photo>("id", &id) {
            if let Some(size) = match req.param("size").unwrap() {
                "s" => Some(200),
                "m" => Some(800),
                "l" => Some(1200),
                _ => None
            } {
                let buf = req.photos().get_scaled_image(photo, size, size);
                res.set(MediaType::Jpeg);
                res.set(Expires(HttpDate(time::now() + Duration::days(14))));
                return res.send(buf);
            }
        }
    }
    res.error(StatusCode::NotFound, "No such image")
}

fn tag_all<'mw>(req: &mut Request, res: Response<'mw>)
                -> MiddlewareResult<'mw>  {
    return render!(res, "templates/tags.tpl", {
        user: Option<String> = req.authorized_user(),
        tags: Vec<Tag> = query_for::<Tag>().asc("tag")
            .collect(req.db_conn()).unwrap()
    });
}
fn tag_one<'mw>(req: &mut Request, res: Response<'mw>)
                -> MiddlewareResult<'mw>  {
    let slug = req.param("tag").unwrap();
    if let Ok(tag) = req.orm_get::<Tag>("slug", &slug) {
        return render!(res, "templates/tag.tpl", {
            user: Option<String> = req.authorized_user(),
            photos: Vec<Photo> =
                req.orm_get_related(&tag, "photo_tag").unwrap(),
            tag: Tag = tag
        });
    }
    res.error(StatusCode::NotFound, "Not a tag")
}

fn place_all<'mw>(req: &mut Request, res: Response<'mw>)
                -> MiddlewareResult<'mw>  {
    return render!(res, "templates/places.tpl", {
        user: Option<String> = req.authorized_user(),
        places: Vec<Place> = query_for::<Place>().asc("place")
            .collect(req.db_conn()).unwrap()
    });
}
fn place_one<'mw>(req: &mut Request, res: Response<'mw>)
                -> MiddlewareResult<'mw>  {
    let slug = req.param("slug").unwrap();
    if let Ok(place) = req.orm_get::<Place>("slug", &slug) {
        return render!(res, "templates/place.tpl", {
            user: Option<String> = req.authorized_user(),
            photos: Vec<Photo> =
                req.orm_get_related(&place, "photo_place").unwrap(),
            place: Place = place
        });
    }
    res.error(StatusCode::NotFound, "Not a place")
}

fn person_all<'mw>(req: &mut Request, res: Response<'mw>)
                -> MiddlewareResult<'mw>  {
    return render!(res, "templates/people.tpl", {
        user: Option<String> = req.authorized_user(),
        people: Vec<Person> = query_for::<Person>().asc("name")
            .collect(req.db_conn()).unwrap()
    });
}
fn person_one<'mw>(req: &mut Request, res: Response<'mw>)
                -> MiddlewareResult<'mw>  {
    let slug = req.param("slug").unwrap();
    if let Ok(person) = req.orm_get::<Person>("slug", &slug) {
        return render!(res, "templates/person.tpl", {
            user: Option<String> = req.authorized_user(),
            photos: Vec<Photo> =
                orm_get_related::<Photo, Person>(&person, "photo_person")
                .desc_nulls_last("grade")
                .desc_nulls_last("date")
                .collect(req.db_conn()).unwrap(),
            person: Person = person
        });
    }
    res.error(StatusCode::NotFound, "Not a place")
}

fn photo_details<'mw>(req: &mut Request, res: Response<'mw>)
                      -> MiddlewareResult<'mw>  {
    if let Ok(id) = req.param("id").unwrap().parse::<i32>() {
        if let Ok(photo) = req.orm_get::<Photo>("id", &id) {
            return render!(res, "templates/details.tpl", {
                user: Option<String> = req.authorized_user(),
                people: Vec<Person> =
                    req.orm_get_related(&photo, "photo_person").unwrap(),
                places: Vec<Place> =
                    req.orm_get_related(&photo, "photo_place").unwrap(),
                tags: Vec<Tag> =
                    req.orm_get_related(&photo, "photo_tag").unwrap(),
                time: String = match photo.date {
                    Some(d) => d.format("%T").to_string(),
                    None => "".to_string()
                },
                year: i32 = match photo.date {
                    Some(d) => d.year(),
                    None => 0
                },
                month: u32 = match photo.date {
                    Some(d) => d.month(),
                    None => 0
                },
                day: u32 = match photo.date {
                    Some(d) => d.day(),
                    None => 0
                },
                photo: Photo = photo
            });
        }
    }
    res.error(StatusCode::NotFound, "Not a year")
}

fn all_years<'mw>(req: &mut Request, res: Response<'mw>) -> MiddlewareResult<'mw>  {
    return render!(res, "templates/groups.tpl", {
        user: Option<String> = req.authorized_user(),
        title: &'static str = "All photos",
        groups: Vec<Group> = query_for::<Photo>()
            .columns(vec!("extract(year from date) y", "count(*) c"))
            .add_filter(Filter::is_not_null("date"))
            .group_by(vec!("y")).asc("y")
            .retrieve(req.db_conn()).expect("Get images per year")
            .dao.iter().map(|dao| {
                debug!("Got a pregroup: {:?}", dao);
                let year = dao.get::<f64>("y") as u16;
                let count : i64 = dao.get("c");
                let photo : Photo = query_for::<Photo>()
                    .filter_eq("extract(year from date)", &(year as f64))
                    .desc_nulls_last("grade")
                    .asc_nulls_last("date")
                    .limit(1)
                    .collect_one(req.db_conn()).unwrap();
                Group {
                    title: format!("{}", year),
                    url: format!("/{}/", year),
                    count: count,
                    photo: photo
                }
            }).collect()
    });
}

fn months_in_year<'mw>(req: &mut Request, res: Response<'mw>) -> MiddlewareResult<'mw>  {
    if let Ok(year) = req.param("year").unwrap().parse::<i32>() {
        return render!(res, "templates/groups.tpl", {
            user: Option<String> = req.authorized_user(),
            title: String = format!("Photos from {}", year),
            groups: Vec<Group> = query_for::<Photo>()
                .columns(vec!("extract(month from date) m", "count(*) c"))
                .filter_eq("extract(year from date)", &(year as f64))
                .group_by(vec!("m")).asc("m")
                .retrieve(req.db_conn()).expect("Get images per month")
                .dao.iter().map(|dao| {
                    let month = dao.get::<f64>("m") as u8;
                    let count : i64 = dao.get("c");
                    let photo : Photo = query_for::<Photo>()
                        .filter_eq("extract(year from date)", &(year as f64))
                        .filter_eq("extract(month from date)", &(month as f64))
                        .desc_nulls_last("grade")
                        .asc_nulls_last("date")
                        .limit(1)
                        .collect_one(req.db_conn()).unwrap();
                    Group {
                        title: monthname(month).to_string(),
                        url: format!("/{}/{}/", year, month),
                        count: count,
                        photo: photo
                    }
                }).collect()
        });
    }
    res.error(StatusCode::NotFound, "Not a year")
}

fn days_in_month<'mw>(req: &mut Request, res: Response<'mw>) -> MiddlewareResult<'mw>  {
    if let Ok(year) = req.param("year").unwrap().parse::<i32>() {
        if let Ok(month) = req.param("month").unwrap().parse::<u8>() {
            return render!(res, "templates/groups.tpl", {
                user: Option<String> = req.authorized_user(),
                title: String = format!("Photos from {} {}", monthname(month),
                                        year),
                groups: Vec<Group> = query_for::<Photo>()
                    .columns(vec!("extract(day from date) d", "count(*) c"))
                    .filter_eq("extract(year from date)", &(year as f64))
                    .filter_eq("extract(month from date)", &(month as f64))
                    .group_by(vec!("d")).asc("d")
                    .retrieve(req.db_conn()).expect("Get images per day")
                    .dao.iter().map(|dao| {
                        let day = dao.get::<f64>("d") as u8;
                        let count : i64 = dao.get("c");
                        let photo : Photo = query_for::<Photo>()
                            .filter_eq("extract(year from date)", &(year as f64))
                            .filter_eq("extract(month from date)", &(month as f64))
                            .filter_eq("extract(day from date)", &(day as f64))
                            .desc_nulls_last("grade")
                            .asc_nulls_last("date")
                            .limit(1)
                            .collect_one(req.db_conn()).unwrap();
                        Group {
                            title: format!("{}/{}", day, month),
                            url: format!("/{}/{}/{}", year, month, day),
                            count: count,
                            photo: photo
                        }
                    }).collect()
            });
        }
    }
    res.error(StatusCode::NotFound, "Not a month")
}

fn all_for_day<'mw>(req: &mut Request, res: Response<'mw>) -> MiddlewareResult<'mw>  {
    if let Ok(year) = req.param("year").unwrap().parse::<i32>() {
        if let Ok(month) = req.param("month").unwrap().parse::<u8>() {
            if let Ok(day) = req.param("day").unwrap().parse::<u32>() {
                let date = UTC.ymd(year, month as u32, day).and_hms(0,0,0);
                return render!(res, "templates/index.tpl", {
                    user: Option<String> = req.authorized_user(),
                    title: String = format!("Photos from {} {} {}",
                                            day, monthname(month), year),
                    photos: Vec<Photo> = query_for::<Photo>()
                        .filter_gte("date", &date)
                        .filter_lt("date", &(date + ChDuration::days(1)))
                        .desc_nulls_last("grade")
                        .asc_nulls_last("date")
                        .collect(req.db_conn()).unwrap()
                })
            }
        }
    }
    res.error(StatusCode::NotFound, "Not a day")
}
