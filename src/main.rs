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

mod models;
use hyper::header::{Expires, HttpDate};
use image::open as image_open;
use image::{FilterType, ImageFormat, GenericImage};
use models::{Entity, Photo, Tag, Person, Place, query_for};
use nickel::{MediaType, Nickel, StaticFilesHandler};
use plugin::{Pluggable};
use rustc_serialize::Encodable;
use rustorm::query::Query;
use std::path::PathBuf;
use time::Duration;

mod env;
use env::{dburl, env_or, photos_dir};

mod rustormmiddleware;
use rustormmiddleware::{RustormMiddleware, RustormRequestExtensions};

mod requestloggermiddleware;
use requestloggermiddleware::RequestLoggerMiddleware;

struct PhotosDir {
    basedir: PathBuf
}

impl PhotosDir {
    fn new(basedir: PathBuf) -> PhotosDir {
        PhotosDir {
            basedir: basedir
        }
    }

    fn get_scaled_image(&self, photo: Photo, width: u32, height: u32)
                        -> Vec<u8> {
        let path = self.basedir.join(photo.path);
        info!("Should open {:?}", path);
        let img = image_open(path).unwrap();
        let img =
            if width < img.width() || height < img.height() {
                img.resize(width, height, FilterType::Nearest)
            } else {
                img
            };
        let img = match photo.rotation {
            _x @ 0...44 => img,
            _x @ 45...134 => img.rotate90(),
            _x @ 135...224 => img.rotate180(),
            _x @ 225...314 => img.rotate270(),
            _x @ 315...360 => img,
            x => {
                warn!("Should rotate photo {} deg, which is unsupported", x);
                img
            }
        };
        // TODO Put the icon in some kind of cache!
        let mut buf : Vec<u8> = Vec::new();
        img.save(&mut buf, ImageFormat::JPEG).unwrap();
        buf
    }
}

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
            return render!(res, "templates/index.tpl", {
                photos: Vec<Photo> = query_for::<Photo>()
                    .desc_nulls_last("grade")
                    .desc_nulls_last("date")
                    .limit(24)
                    .collect(req.db_conn()).unwrap()
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
                        date: String =
                            if let Some(d) = photo.date {
                                d.to_rfc3339()
                            } else {
                                "".to_string()
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
    });

    server.listen(&*env_or("RPHOTOS_LISTEN", "127.0.0.1:6767"));
}
