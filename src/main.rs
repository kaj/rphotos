#[macro_use] extern crate nickel;
#[macro_use] extern crate log;
extern crate env_logger;
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
use nickel::{MediaType, Nickel, StaticFilesHandler};
use plugin::{Pluggable};
use rustc_serialize::Encodable;
use rustorm::query::{Query, Filter};
use time::Duration;

mod models;
use models::{Entity, Photo, Tag, Person, Place, query_for};

mod env;
use env::{dburl, env_or, photos_dir};

mod photosdir;
use photosdir::PhotosDir;

mod rustormmiddleware;
use rustormmiddleware::{RustormMiddleware, RustormRequestExtensions};

mod requestloggermiddleware;
use requestloggermiddleware::RequestLoggerMiddleware;


macro_rules! render {
    ($res:expr, $template:expr, { $($param:ident : $ptype:ty = $value:expr),* })
        =>
    {
        {
        #[derive(Debug, Clone, RustcEncodable)]
        struct ParamData {
            $(
                $param: $ptype,
                )*
        }
        $res.render($template, &ParamData {
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

    let photos = PhotosDir::new(photos_dir());
    let mut server = Nickel::new();
    server.utilize(RequestLoggerMiddleware);
    server.utilize(StaticFilesHandler::new("static/"));
    server.utilize(RustormMiddleware::new(&dburl()));
    server.utilize(router! {
        get "/" => |req, res| {
            return render!(res, "templates/groups.tpl", {
                title: &'static str = "All photos",
                groups: Vec<Group> = {
                    query_for::<Photo>()
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
                }
            });
        }
        get "/details/:id" => |req, res| {
            if let Ok(id) = req.param("id").unwrap().parse::<i32>() {
                if let Ok(photo) = req.orm_get::<Photo>("id", &id) {
                    return render!(res, "templates/details.tpl", {
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
        }
        get "/tag/" => |req, res| {
            return render!(res, "templates/tags.tpl", {
                tags: Vec<Tag> = query_for::<Tag>().asc("tag")
                    .collect(req.db_conn()).unwrap()
            });
        }
        get "/tag/:tag" => |req, res| {
            let slug = req.param("tag").unwrap();
            if let Ok(tag) = req.orm_get::<Tag>("slug", &slug) {
                return render!(res, "templates/tag.tpl", {
                    photos: Vec<Photo> =
                        req.orm_get_related(&tag, "photo_tag").unwrap(),
                    tag: Tag = tag
                });
            }
        }
        get "/person/" => |req, res| {
            return render!(res, "templates/people.tpl", {
                people: Vec<Person> = query_for::<Person>().asc("name")
                    .collect(req.db_conn()).unwrap()
            });
        }
        get "/person/:slug" => |req, res| {
            let slug = req.param("slug").unwrap();
            if let Ok(person) = req.orm_get::<Person>("slug", &slug) {
                return render!(res, "templates/person.tpl", {
                    photos: Vec<Photo> =
                        orm_get_related::<Photo, Person>(&person, "photo_person")
                        .desc_nulls_last("grade")
                        .desc_nulls_last("date")
                        .collect(req.db_conn()).unwrap(),
                    person: Person = person
                });
            }
        }
        get "/place/" => |req, res| {
            return render!(res, "templates/places.tpl", {
                places: Vec<Place> = query_for::<Place>().asc("place")
                    .collect(req.db_conn()).unwrap()
            });
        }
        get "/place/:slug" => |req, res| {
            let slug = req.param("slug").unwrap();
            if let Ok(place) = req.orm_get::<Place>("slug", &slug) {
                return render!(res, "templates/place.tpl", {
                    photos: Vec<Photo> =
                        req.orm_get_related(&place, "photo_place").unwrap(),
                    place: Place = place
                });
            }
        }
        get "/img/:id/:size" => |req, mut res| {
            if let Ok(id) = req.param("id").unwrap().parse::<i32>() {
                if let Ok(photo) = req.orm_get::<Photo>("id", &id) {
                    if let Some(size) = match req.param("size").unwrap() {
                        "s" => Some(200),
                        "m" => Some(800),
                        "l" => Some(1200),
                        _ => None
                    } {
                        let buf = photos.get_scaled_image(photo, size, size);
                        res.set(MediaType::Jpeg);
                        res.set(Expires(HttpDate(time::now() + Duration::days(14))));
                        return res.send(buf);
                    }
                }
            }
        }
        get "/:year/" => |req, res| {
            if let Ok(year) = req.param("year").unwrap().parse::<i32>() {
                return render!(res, "templates/groups.tpl", {
                    title: String = format!("Photos from {}", year),
                    groups: Vec<Group> = {
                        let m = query_for::<Photo>()
                            .columns(vec!("extract(month from date) m", "count(*) c"))
                            .filter_eq("extract(year from date)", &(year as f64))
                            .group_by(vec!("m")).asc("m")
                            .retrieve(req.db_conn()).expect("Get images per month");
                        m.dao.iter().map(|dao| {
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
                    }
                });
            }
        }
        get "/:year/:month/" => |req, res| {
            if let Ok(year) = req.param("year").unwrap().parse::<i32>() {
                if let Ok(month) = req.param("month").unwrap().parse::<u8>() {
                    return render!(res, "templates/groups.tpl", {
                        title: String = format!("Photos from {} {}", monthname(month),
                                                year),
                        groups: Vec<Group> = {
                            let m = query_for::<Photo>()
                                .columns(vec!("extract(day from date) d", "count(*) c"))
                                .filter_eq("extract(year from date)", &(year as f64))
                                .filter_eq("extract(month from date)", &(month as f64))
                                .group_by(vec!("d")).asc("d")
                                .retrieve(req.db_conn()).expect("Get images per day");
                            m.dao.iter().map(|dao| {
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
                        }
                    });
                }
            }
        }
        get "/:year/:month/:day" => |req, res| {
            if let Ok(year) = req.param("year").unwrap().parse::<i32>() {
                if let Ok(month) = req.param("month").unwrap().parse::<u8>() {
                    if let Ok(day) = req.param("day").unwrap().parse::<u32>() {
                        let date = UTC.ymd(year, month as u32, day).and_hms(0,0,0);
                        return render!(res, "templates/index.tpl", {
                            title: String = format!("Photos from {} {} {}",
                                                    day, monthname(month), year),
                            photos: Vec<Photo> = query_for::<Photo>()
                                .filter_gte("date", &date)
                                .filter_lt("date", &(date + ChDuration::days(1)))
                                .desc_nulls_last("grade")
                                .asc_nulls_last("date")
                                .collect(req.db_conn()).unwrap()
                        });
                    }
                }
            }
        }
    });

    server.listen(&*env_or("RPHOTOS_LISTEN", "127.0.0.1:6767"));
}
