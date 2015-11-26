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
use models::{Photo, Tag, query_for};
use nickel::mimes::MediaType;
use nickel::{Nickel, Request, Response, Middleware, Continue, MiddlewareResult};
use plugin::{Pluggable, Extensible};
use rustc_serialize::Encodable;
use rustorm::dao::{IsDao, ToValue};
use rustorm::database::{Database, DbError};
use rustorm::pool::{ManagedPool,Platform};
use rustorm::query::Query;
use rustorm::table::IsTable;
use std::collections::HashMap;
use time::Duration;
use typemap::Key;

mod env;
use env::dburl;

struct RustormMiddleware {
    pool: ManagedPool
}

impl RustormMiddleware {
    pub fn new(db_url: &str) -> RustormMiddleware {
        RustormMiddleware {
            pool: ManagedPool::init(db_url, 5).unwrap(),
        }
    }
}

impl Key for RustormMiddleware { type Value = Platform; }

impl<D> Middleware<D> for RustormMiddleware {
    fn invoke<'mw, 'conn>(&self, req: &mut Request<'mw, 'conn, D>, res: Response<'mw, D>) -> MiddlewareResult<'mw, D> {
        req.extensions_mut().insert::<RustormMiddleware>(
            self.pool.connect().unwrap());
        Ok(Continue(res))
    }
}

pub trait RustormRequestExtensions {
    fn db_conn(&self) -> &Database;
    fn orm_get<T: IsTable + IsDao>(&self, key: &str, val: &ToValue)
                                   -> Result<T, DbError>;
}

impl<'a, 'b, D> RustormRequestExtensions for Request<'a, 'b, D> {
    fn db_conn(&self) -> &Database {
        self.extensions().get::<RustormMiddleware>().unwrap().as_ref()
    }
    fn orm_get<T: IsTable + IsDao>(&self, key: &str, val: &ToValue)
                                   -> Result<T, DbError> {
        query_for::<T>().filter_eq(key, val).collect_one(self.db_conn())
    }
}

#[derive(Debug, Clone, RustcEncodable)]
struct DetailsData {
    photo: Photo,
    tags: Vec<Tag>
}

#[derive(Debug, Clone, RustcEncodable)]
struct TagData {
    tag: Tag,
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
    server.utilize(RustormMiddleware::new(&dburl()));

    server.utilize(router! {
        get "/" => |req, res| {
            let photos: Vec<Photo> = query_for::<Photo>().limit(16)
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

                    return res.render("templates/details.tpl", &DetailsData {
                        photo: photo,
                        tags: tags
                    });
                }
            }
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
