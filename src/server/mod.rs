#[macro_use]
mod nickelext;
mod views_by_date;

use adm::result::Error;
use chrono::Datelike;
use clap::ArgMatches;
use diesel::pg::PgConnection;
use diesel;
use diesel::prelude::*;
use djangohashers;
use hyper::header::ContentType;
use image;
use memcachemiddleware::MemcacheMiddleware;
use nickel::{Action, Continue, FormBody, Halt, HttpRouter, MediaType,
             MiddlewareResult, Nickel, NickelError, QueryString, Request,
             Response};
use nickel::extensions::response::Redirect;
use nickel::status::StatusCode::NotFound;
use nickel_diesel::{DieselMiddleware, DieselRequestExtensions};
use nickel_jwt_session::{SessionMiddleware, SessionRequestExtensions,
                         SessionResponseExtensions};
use r2d2::NopErrorHandler;
use rphotos::models::{Person, Photo, Place, Tag};

use env::{dburl, env_or, jwt_key, photos_dir};

use requestloggermiddleware::RequestLoggerMiddleware;

use photosdirmiddleware::{PhotosDirMiddleware, PhotosDirRequestExtensions};

use memcachemiddleware::*;

use pidfiles::handle_pid_file;
use templates;

use self::nickelext::{FromSlug, MyResponse, far_expires};

use self::views_by_date::*;

#[derive(Debug, Clone)]
pub struct Group {
    pub title: String,
    pub url: String,
    pub count: i64,
    pub photo: Photo,
}

#[derive(Debug, Clone)]
pub struct Coord {
    pub x: f64,
    pub y: f64,
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    if let Some(pidfile) = args.value_of("PIDFILE") {
        handle_pid_file(pidfile, args.is_present("REPLACE"))
            .unwrap()
    }

    let mut server = Nickel::new();
    server.utilize(RequestLoggerMiddleware);
    wrap3!(server.get "/static/{}\\.{}", static_file: file, ext);
    server.utilize(MemcacheMiddleware::new
                   (vec![("tcp://127.0.0.1:11211".into(), 1)]));
    server.utilize(SessionMiddleware::new(&jwt_key()));
    let dm: DieselMiddleware<PgConnection> =
        DieselMiddleware::new(&dburl(), 5, Box::new(NopErrorHandler)).unwrap();
    server.utilize(dm);
    server.utilize(PhotosDirMiddleware::new(photos_dir()));

    wrap3!(server.get  "/login",           login);
    wrap3!(server.post "/login",           do_login);
    wrap3!(server.get  "/logout",          logout);
    wrap3!(server.get "/",                 all_years);
    wrap3!(server.get "/img/{}[-]{}\\.jpg", show_image: id, size);
    wrap3!(server.get "/img/{}",           photo_details: id);
    wrap3!(server.get "/tag/",             tag_all);
    wrap3!(server.get "/tag/{}",           tag_one: tag);
    wrap3!(server.get "/place/",           place_all);
    wrap3!(server.get "/place/{}",         place_one: slug);
    wrap3!(server.get "/person/",          person_all);
    wrap3!(server.get "/person/{}",        person_one: slug);
    wrap3!(server.get "/0/",               all_null_date);
    wrap3!(server.get "/{}/",              months_in_year: year);
    wrap3!(server.get "/{}/{}/",           days_in_month: year, month);
    wrap3!(server.get "/{}/{}/{}",         all_for_day: year, month, day);
    wrap3!(server.get "/{}/{}/{}/{}",      part_for_day: year, month, day, part);
    wrap3!(server.get "/thisday",          on_this_day);

    // https://github.com/rust-lang/rust/issues/20178
    let custom_handler: fn(&mut NickelError, &mut Request) -> Action = custom_errors;
    server.handle_error(custom_handler);

    server.listen(&*env_or("RPHOTOS_LISTEN", "127.0.0.1:6767"))
        .map_err(|e| Error::Other(format!("listen: {}", e)))?;
    Ok(())
}


fn custom_errors(err: &mut NickelError, req: &mut Request) -> Action {
    if let Some(ref mut res) = err.stream {
        if res.status() == NotFound {
            templates::not_found(res, req).unwrap();
            return Halt(());
        }
    }

    Continue(())
}

fn login<'mw>(req: &mut Request,
              mut res: Response<'mw>)
              -> MiddlewareResult<'mw> {
    res.clear_jwt();
    let next = sanitize_next(req.query().get("next")).map(String::from);
    res.ok(|o| templates::login(o, req, next))
}

fn do_login<'mw>(req: &mut Request,
                 mut res: Response<'mw>)
                 -> MiddlewareResult<'mw> {
    let next = {
    let c: &PgConnection = &req.db_conn();
    let form_data = try_with!(res, req.form_body());
    let next = sanitize_next(form_data.get("next")).map(String::from);
    if let (Some(user), Some(pw)) = (form_data.get("user"),
                                     form_data.get("password")) {
        use rphotos::schema::users::dsl::*;
        if let Ok(hash) = users.filter(username.eq(user))
                               .select(password)
                               .first::<String>(c) {
            debug!("Hash for {} is {}", user, hash);
            if djangohashers::check_password_tolerant(pw, &hash) {
                info!("User {} logged in", user);
                res.set_jwt_user(user);
                return res.redirect(next.unwrap_or("/".to_string()));
            }
            debug!("Password verification failed");
        } else {
            debug!("No hash found for {}", user);
        }
    }
        next
    };
    res.ok(|o| templates::login(o, req, next))
}

fn sanitize_next(next: Option<&str>) -> Option<&str> {
    if let Some(next) = next {
        use regex::Regex;
        let re = Regex::new(r"^/([a-z0-9.-]+/?)*$").unwrap();
        if re.is_match(next) {
            return Some(next)
        }
    }
    None
}

#[test]
fn test_sanitize_bad_1() {
    assert_eq!(None, sanitize_next(Some("https://evil.org/")))
}

#[test]
fn test_sanitize_bad_2() {
    assert_eq!(None, sanitize_next(Some("//evil.org/")))
}
#[test]
fn test_sanitize_bad_3() {
    assert_eq!(None, sanitize_next(Some("/evil\"hack")))
}
#[test]
fn test_sanitize_bad_4() {
    assert_eq!(None, sanitize_next(Some("/evil'hack")))
}

#[test]
fn test_sanitize_good_1() {
    assert_eq!(Some("/foo/"), sanitize_next(Some("/foo/")))
}
#[test]
fn test_sanitize_good_2() {
    assert_eq!(Some("/2017/7/15"), sanitize_next(Some("/2017/7/15")))
}

fn logout<'mw>(_req: &mut Request,
               mut res: Response<'mw>)
               -> MiddlewareResult<'mw> {
    res.clear_jwt();
    res.redirect("/")
}

#[derive(Debug, PartialEq, Eq)]
enum SizeTag {
    Small,
    Medium,
    Large,
}
impl SizeTag {
    fn px(&self) -> u32 {
        match *self {
            SizeTag::Small => 240,
            SizeTag::Medium => 960,
            SizeTag::Large => 1900,
        }
    }
}

impl FromSlug for SizeTag {
    fn parse(slug: &str) -> Option<Self> {
        match slug {
            "s" => Some(SizeTag::Small),
            "m" => Some(SizeTag::Medium),
            "l" => Some(SizeTag::Large),
            _ => None,
        }
    }
}

fn show_image<'mw>(req: &Request,
                   mut res: Response<'mw>,
                   the_id: i32,
                   size: SizeTag)
                   -> MiddlewareResult<'mw> {
    use rphotos::schema::photos::dsl::photos;
    let c: &PgConnection = &req.db_conn();
    if let Ok(tphoto) = photos.find(the_id).first::<Photo>(c) {
        if req.authorized_user().is_some() || tphoto.is_public() {
            if size == SizeTag::Large {
                if req.authorized_user().is_some() {
                    let path = req.photos().get_raw_path(tphoto);
                    res.set((MediaType::Jpeg, far_expires()));
                    return res.send_file(path);
                }
            } else {
                let data = get_image_data(req, tphoto, size)
                    .expect("Get image data");
                res.set((MediaType::Jpeg, far_expires()));
                return res.send(data);
            }
        }
    }
    res.not_found("No such image")
}

fn get_image_data(req: &Request,
                  photo: Photo,
                  size: SizeTag)
                  -> Result<Vec<u8>, image::ImageError>
{
    req.cached_or(&format!("rp{}{:?}", photo.id, size), || {
        let size = size.px();
        req.photos().scale_image(photo, size, size)
    })
}

fn tag_all<'mw>(req: &mut Request,
                res: Response<'mw>)
                -> MiddlewareResult<'mw> {
    use rphotos::schema::tags::dsl::{id, tag_name, tags};
    let c: &PgConnection = &req.db_conn();
    let query = tags.into_boxed();
    let query = if req.authorized_user().is_some() {
        query
    } else {
        use rphotos::schema::photo_tags::dsl as tp;
        use rphotos::schema::photos::dsl as p;
        query.filter(id.eq_any(tp::photo_tags
                               .select(tp::tag_id)
                               .filter(tp::photo_id
                                       .eq_any(p::photos
                                               .select(p::id)
                                               .filter(p::is_public)))))
    };
    res.ok(|o| {
        templates::tags(o,
                        req,
                        &query.order(tag_name).load(c).expect("List tags"))
    })
}

fn tag_one<'mw>(req: &mut Request,
                res: Response<'mw>,
                tslug: String)
                -> MiddlewareResult<'mw> {
    use rphotos::schema::tags::dsl::{slug, tags};
    let c: &PgConnection = &req.db_conn();
    if let Ok(tag) = tags.filter(slug.eq(tslug)).first::<Tag>(c) {
        use rphotos::schema::photos::dsl::{date, grade, id};
        use rphotos::schema::photo_tags::dsl::{photo_id, photo_tags, tag_id};
        return res.ok(|o| {
            templates::tag(o,
                           req,
                           &Photo::query(req.authorized_user().is_some())
                           .filter(id.eq_any(photo_tags.select(photo_id)
                                             .filter(tag_id.eq(tag.id))))
                           .order((grade.desc().nulls_last(),
                                   date.desc().nulls_last()))
                           .load(c).unwrap(),
                           tag)
        })
    }
    res.not_found("Not a tag")
}

fn place_all<'mw>(req: &mut Request,
                  res: Response<'mw>)
                  -> MiddlewareResult<'mw> {
    use rphotos::schema::places::dsl::{id, place_name, places};
    let query = places.into_boxed();
    let query = if req.authorized_user().is_some() {
        query
    } else {
        use rphotos::schema::photo_places::dsl as pp;
        use rphotos::schema::photos::dsl as p;
        query.filter(id.eq_any(pp::photo_places
                               .select(pp::place_id)
                               .filter(pp::photo_id
                                       .eq_any(p::photos
                                               .select(p::id)
                                               .filter(p::is_public)))))
    };
    let c: &PgConnection = &req.db_conn();
    res.ok(|o| templates::places(
        o,
        req,
        &query.order(place_name).load(c).expect("List places")))
}

fn static_file<'mw>(_req: &mut Request,
                    mut res: Response<'mw>,
                    name: String,
                    ext: String)
                    -> MiddlewareResult<'mw> {
    use templates::statics::StaticFile;
    if let Some(s) = StaticFile::get(&format!("{}.{}", name, ext)) {
        res.set((ContentType(s.mime()), far_expires()));
        return res.send(s.content);
    }
    res.not_found("No such file")
}

fn place_one<'mw>(req: &mut Request,
                  res: Response<'mw>,
                  tslug: String)
                  -> MiddlewareResult<'mw> {
    use rphotos::schema::places::dsl::{places, slug};
    let c: &PgConnection = &req.db_conn();
    if let Ok(place) = places.filter(slug.eq(tslug)).first::<Place>(c) {
        use rphotos::schema::photos::dsl::{date, grade, id};
        use rphotos::schema::photo_places::dsl::{photo_id, photo_places,
                                                 place_id};
        return res.ok(|o| templates::place(
            o,
            req,
            &Photo::query(req.authorized_user().is_some())
                .filter(id.eq_any(photo_places.select(photo_id)
                                              .filter(place_id.eq(place.id))))
                .order((grade.desc().nulls_last(), date.desc().nulls_last()))
                .load(c).unwrap(),
            place));
    }
    res.not_found("Not a place")
}

fn person_all<'mw>(req: &mut Request,
                   res: Response<'mw>)
                   -> MiddlewareResult<'mw> {
    use rphotos::schema::people::dsl::{id, people, person_name};
    let query = people.into_boxed();
    let query = if req.authorized_user().is_some() {
        query
    } else {
        use rphotos::schema::photo_people::dsl as pp;
        use rphotos::schema::photos::dsl as p;
        query.filter(id.eq_any(pp::photo_people
                               .select(pp::person_id)
                               .filter(pp::photo_id
                                       .eq_any(p::photos
                                               .select(p::id)
                                               .filter(p::is_public)))))
    };
    let c: &PgConnection = &req.db_conn();
    res.ok(|o| templates::people(
        o,
        req,
        &query.order(person_name).load(c).expect("list people")))
}

fn person_one<'mw>(req: &mut Request,
                   res: Response<'mw>,
                   tslug: String)
                   -> MiddlewareResult<'mw> {
    use rphotos::schema::people::dsl::{people, slug};
    let c: &PgConnection = &req.db_conn();
    if let Ok(person) = people.filter(slug.eq(tslug)).first::<Person>(c) {
        use rphotos::schema::photos::dsl::{date, grade, id};
        use rphotos::schema::photo_people::dsl::{person_id, photo_id,
                                                 photo_people};
        return res.ok(|o| templates::person(
            o,
            req,
            &Photo::query(req.authorized_user().is_some())
                .filter(id.eq_any(photo_people.select(photo_id)
                                              .filter(person_id.eq(person.id))))
                .order((grade.desc().nulls_last(), date.desc().nulls_last()))
                .load(c).unwrap(),
            person));
    }
    res.not_found("Not a person")
}

fn photo_details<'mw>(req: &mut Request,
                      res: Response<'mw>,
                      id: i32)
                      -> MiddlewareResult<'mw> {
    use rphotos::schema::photos::dsl::photos;
    let c: &PgConnection = &req.db_conn();
    if let Ok(tphoto) = photos.find(id).first::<Photo>(c) {
        if req.authorized_user().is_some() || tphoto.is_public() {
            return res.ok(|o| templates::details(
                o,
                req,
                &tphoto.date
                    .map(|d| vec![Link::year(d.year()),
                                  Link::month(d.year(), d.month()),
                                  Link::day(d.year(), d.month(), d.day())])
                    .unwrap_or_else(|| vec![]),
                &{
                    use rphotos::schema::people::dsl::{people, id};
                    use rphotos::schema::photo_people::dsl::{photo_people, photo_id, person_id};
                    people.filter(id.eq_any(photo_people.select(person_id)
                                            .filter(photo_id.eq(tphoto.id))))
                        .load(c).unwrap()
                },
                &{
                    use rphotos::schema::places::dsl::{places, id};
                    use rphotos::schema::photo_places::dsl::{photo_places, photo_id, place_id};
                    places.filter(id.eq_any(photo_places.select(place_id)
                                            .filter(photo_id.eq(tphoto.id))))
                        .load(c).unwrap()
                },
                &{
                    use rphotos::schema::tags::dsl::{tags, id};
                    use rphotos::schema::photo_tags::dsl::{photo_tags, photo_id, tag_id};
                    tags.filter(id.eq_any(photo_tags.select(tag_id)
                                          .filter(photo_id.eq(tphoto.id))))
                        .load(c).unwrap()
                },
                {
                    use rphotos::schema::positions::dsl::*;
                    match positions.filter(photo_id.eq(tphoto.id))
                        .select((latitude, longitude))
                        .first::<(i32, i32)>(c) {
                            Ok((tlat, tlong)) => Some(Coord {
                                x: tlat as f64 / 1e6,
                                y: tlong as f64 / 1e6,
                            }),
                            Err(diesel::NotFound) => None,
                            Err(err) => {
                                error!("Failed to read position: {}", err);
                                None
                            }
                        }
                },
                {
                    use rphotos::schema::attributions::dsl::*;
                    tphoto.attribution_id.map(|i| {
                        attributions.find(i).select(name).first(c).unwrap()
                    })
                },
                {
                    use rphotos::schema::cameras::dsl::*;
                    tphoto.camera_id.map(|i| {
                        cameras.find(i).first(c).unwrap()
                    })
                },
                tphoto));
        }
    }
    res.not_found("Photo not found")
}


#[derive(Debug, Clone)]
pub struct Link {
    pub url: String,
    pub name: String,
}

impl Link {
    fn year(year: i32) -> Self {
        Link {
            url: format!("/{}/", year),
            name: format!("{}", year),
        }
    }
    fn month(year: i32, month: u32) -> Self {
        Link {
            url: format!("/{}/{}/", year, month),
            name: format!("{}", month),
        }
    }
    fn day(year: i32, month: u32, day: u32) -> Self {
        Link {
            url: format!("/{}/{}/{}", year, month, day),
            name: format!("{}", day),
        }
    }
}