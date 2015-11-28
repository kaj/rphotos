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
use image::{FilterType, ImageFormat};
use models::{Photo, Tag, Person, query_for};
use nickel::{MediaType, Nickel, StaticFilesHandler};
use plugin::{Pluggable};
use rustc_serialize::Encodable;
use rustorm::database::{Database};
use rustorm::query::Query;
use rustorm::table::IsTable;
use std::collections::HashMap;
use time::Duration;

mod env;
use env::dburl;

mod rustormmiddleware;
use rustormmiddleware::{RustormMiddleware, RustormRequestExtensions};

#[derive(Debug, Clone, RustcEncodable)]
struct DetailsData {
    photo: Photo,
    people: Vec<Person>,
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

fn get_scaled_image(photo: Photo, width: u32, height: u32) -> Vec<u8> {
    let path = format!("/home/kaj/Bilder/foto/{}", photo.path);
    info!("Should open {}", path);
    let img = image_open(path).unwrap();
    let scaled = img.resize(width, height, FilterType::Nearest);
    // TODO Put the icon in some kind of cache!
    let mut buf : Vec<u8> = Vec::new();
    scaled.save(&mut buf, ImageFormat::JPEG).unwrap();
    buf
}

fn main() {
    env_logger::init().unwrap();
    info!("Initalized logger");
    // NOTE pool will need to be mut if we do db writes?
    // let pool = ManagedPool::init(&dburl(), 1).unwrap();
    info!("Initalized pool");

    let mut server = Nickel::new();
    server.utilize(StaticFilesHandler::new("static/"));
    server.utilize(RustormMiddleware::new(&dburl()));

    server.utilize(router! {
        get "/" => |req, res| {
            let photos: Vec<Photo> = query_for::<Photo>().limit(25)
                .collect(req.db_conn()).unwrap();
            info!("Got some photos: {:?}", photos);
            let mut data = HashMap::new();
            data.insert("photos", &photos);
            // data.insert("name", "Self".into());
            return res.render("templates/index.tpl", &data);
        }
        get "/details/:id" => |req, res| {
            if let Ok(id) = req.param("id").unwrap().parse::<i32>() {
                if let Ok(photo) = req.orm_get::<Photo>("id", &id) {

                    let mut q = Query::select();
                    q.only_from(&Tag::table());
                    q.left_join_table("photo_tag", "tag.id", "photo_tag.tag")
                        .filter_eq("photo_tag.photo", &photo.id);
                    let tags = q.collect(req.db_conn()).unwrap();

                    let mut q = Query::select();
                    q.only_from(&Person::table());
                    q.left_join_table("photo_person",
                                      "person.id", "photo_person.person")
                        .filter_eq("photo_person.photo", &photo.id);
                    let people = q.collect(req.db_conn()).unwrap();

                    return res.render("templates/details.tpl", &DetailsData {
                        photo: photo,
                        people: people,
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

                let mut q = Query::select();
                q.only_from(&Photo::table());
                q.left_join_table("photo_tag", "photo.id", "photo_tag.photo")
                    .filter_eq("photo_tag.tag", &tag.id);
                let photos : Vec<Photo> = q.collect(req.db_conn()).unwrap();
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

                let mut q = Query::select();
                q.only_from(&Photo::table());
                q.left_join_table("photo_person", "photo.id", "photo_person.photo")
                    .filter_eq("photo_person.person", &person.id);
                let photos : Vec<Photo> = q.collect(req.db_conn()).unwrap();
                return res.render("templates/person.tpl", &PersonData {
                    person: person,
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
