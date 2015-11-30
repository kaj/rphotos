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

mod models;
use hyper::header::{Expires, HttpDate};
use image::open as image_open;
use image::{FilterType, ImageFormat, GenericImage};
use models::{Photo, Tag, Person, Place, query_for};
use nickel::{MediaType, Nickel, StaticFilesHandler};
use plugin::{Pluggable};
use rustc_serialize::Encodable;
use rustorm::database::{Database};
use std::collections::HashMap;
use time::Duration;

mod env;
use env::dburl;

mod rustormmiddleware;
use rustormmiddleware::{RustormMiddleware, RustormRequestExtensions};

mod requestloggermiddleware;
use requestloggermiddleware::RequestLoggerMiddleware;

#[derive(Debug, Clone, RustcEncodable)]
struct DetailsData {
    photo: Photo,
    people: Vec<Person>,
    places: Vec<Place>,
    tags: Vec<Tag>
}

#[derive(Debug, Clone, RustcEncodable)]
struct TagData {
    tag: Tag,
    photos: Vec<Photo>
}

#[derive(Debug, Clone, RustcEncodable)]
struct PersonData {
    person: Person,
    photos: Vec<Photo>
}

#[derive(Debug, Clone, RustcEncodable)]
struct PlaceData {
    place: Place,
    photos: Vec<Photo>
}

fn get_scaled_image(photo: Photo, width: u32, height: u32) -> Vec<u8> {
    let path = format!("/home/kaj/Bilder/foto/{}", photo.path);
    info!("Should open {}", path);
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

fn main() {
    env_logger::init().unwrap();
    info!("Initalized logger");
    // NOTE pool will need to be mut if we do db writes?
    // let pool = ManagedPool::init(&dburl(), 1).unwrap();
    info!("Initalized pool");

    let mut server = Nickel::new();
    server.utilize(RequestLoggerMiddleware);
    server.utilize(StaticFilesHandler::new("static/"));
    server.utilize(RustormMiddleware::new(&dburl()));
    server.utilize(router! {
        get "/" => |req, res| {
            let photos: Vec<Photo> = query_for::<Photo>()
                .filter_gte("grade", &4_i16)
                .limit(24)
                .collect(req.db_conn()).unwrap();
            let mut data = HashMap::new();
            data.insert("photos", &photos);
            info!("About to render for /");
            return res.render("templates/index.tpl", &data);
        }
        get "/details/:id" => |req, res| {
            if let Ok(id) = req.param("id").unwrap().parse::<i32>() {
                if let Ok(photo) = req.orm_get::<Photo>("id", &id) {

                    let people = req.orm_get_related(&photo, "photo_person").unwrap();
                    let places = req.orm_get_related(&photo, "photo_place").unwrap();
                    let tags = req.orm_get_related(&photo, "photo_tag").unwrap();
                    return res.render("templates/details.tpl", &DetailsData {
                        photo: photo,
                        people: people,
                        places: places,
                        tags: tags
                    });
                }
            }
        }
        get "/tag/" => |req, res| {
            let tags: Vec<Tag> = query_for::<Tag>().asc("tag")
                .collect(req.db_conn()).unwrap();
            let mut data = HashMap::new();
            data.insert("tags", &tags);
            return res.render("templates/tags.tpl", &data);
        }
        get "/tag/:tag" => |req, res| {
            let slug = req.param("tag").unwrap();
            if let Ok(tag) = req.orm_get::<Tag>("slug", &slug) {
                let photos = req.orm_get_related(&tag, "photo_tag").unwrap();
                return res.render("templates/tag.tpl", &TagData {
                    tag: tag,
                    photos: photos
                });
            }
        }
        get "/person/" => |req, res| {
            let people: Vec<Person> = query_for::<Person>().asc("name")
                .collect(req.db_conn()).unwrap();
            let mut data = HashMap::new();
            data.insert("people", &people);
            return res.render("templates/people.tpl", &data);
        }
        get "/person/:slug" => |req, res| {
            let slug = req.param("slug").unwrap();
            if let Ok(person) = req.orm_get::<Person>("slug", &slug) {
                let photos = req.orm_get_related(&person, "photo_person").unwrap();
                return res.render("templates/person.tpl", &PersonData {
                    person: person,
                    photos: photos
                });
            }
        }
        get "/place/" => |req, res| {
            let places: Vec<Place> = query_for::<Place>().asc("place")
                .collect(req.db_conn()).unwrap();
            let mut data = HashMap::new();
            data.insert("places", &places);
            return res.render("templates/places.tpl", &data);
        }
        get "/place/:slug" => |req, res| {
            let slug = req.param("slug").unwrap();
            if let Ok(place) = req.orm_get::<Place>("slug", &slug) {
                let photos = req.orm_get_related(&place, "photo_place").unwrap();
                return res.render("templates/place.tpl", &PlaceData {
                    place: place,
                    photos: photos
                });
            }
        }
        get "/icon/:id" => |req, mut res| {
            if let Ok(id) = req.param("id").unwrap().parse::<i32>() {
                if let Ok(photo) = req.orm_get::<Photo>("id", &id) {
                    let buf = get_scaled_image(photo, 200, 180);
                    res.set(MediaType::Jpeg);
                    res.set(Expires(HttpDate(time::now() + Duration::days(14))));
                    return res.send(buf);
                }
            }
        }
        get "/view/:id" => |req, mut res| {
            if let Ok(id) = req.param("id").unwrap().parse::<i32>() {
                if let Ok(photo) = req.orm_get::<Photo>("id", &id) {
                    let buf = get_scaled_image(photo, 800, 600);
                    res.set(MediaType::Jpeg);
                    res.set(Expires(HttpDate(time::now() + Duration::days(14))));
                    return res.send(buf);
                }
            }
        }
    });

    server.listen("127.0.0.1:6767");
}
