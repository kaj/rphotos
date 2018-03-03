#[macro_use]
mod nickelext;
mod admin;
mod splitlist;
mod views_by_date;

use self::nickelext::{far_expires, FromSlug, MyResponse};
use self::splitlist::*;
use self::views_by_date::*;
use adm::result::Error;
use chrono::Datelike;
use clap::ArgMatches;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use djangohashers;
use env::{dburl, env_or, jwt_key, photos_dir};
use hyper::header::ContentType;
use image;
use memcachemiddleware::{MemcacheMiddleware, MemcacheRequestExtensions};
use models::{Person, Photo, Place, Tag};
use nickel::{Action, Continue, FormBody, Halt, HttpRouter, MediaType,
             MiddlewareResult, Nickel, NickelError, QueryString, Request,
             Response};
use nickel::extensions::response::Redirect;
use nickel::status::StatusCode;
use nickel_diesel::{DieselMiddleware, DieselRequestExtensions};
use nickel_jwt_session::{SessionMiddleware, SessionRequestExtensions,
                         SessionResponseExtensions};
use photosdirmiddleware::{PhotosDirMiddleware, PhotosDirRequestExtensions};
use pidfiles::handle_pid_file;
use r2d2::NopErrorHandler;
use requestloggermiddleware::RequestLoggerMiddleware;
use rustc_serialize::json::ToJson;
use templates::{self, Html};

pub struct PhotoLink {
    pub title: Option<String>,
    pub href: String,
    pub id: i32,
    pub lable: Option<String>,
}

impl PhotoLink {
    fn for_group(g: &[Photo], base_url: &str) -> PhotoLink {
        if g.len() == 1 {
            PhotoLink::from(&g[0])
        } else {
            fn imgscore(p: &Photo) -> i16 {
                p.grade.unwrap_or(27) + if p.is_public { 38 } else { 0 }
            }
            PhotoLink {
                title: None,
                href: format!(
                    "{}?from={}&to={}",
                    base_url,
                    g.last().map(|p| p.id).unwrap_or(0),
                    g.first().map(|p| p.id).unwrap_or(0),
                ),
                id: g.iter()
                    .max_by_key(|p| imgscore(p))
                    .map(|p| p.id)
                    .unwrap_or(0),
                lable: {
                    let from = g.last().and_then(|p| p.date);
                    let to = g.first().and_then(|p| p.date);
                    if let (Some(from), Some(to)) = (from, to) {
                        if from.date() == to.date() {
                            Some(format!(
                                "{} - {} ({})",
                                from.format("%F %R"),
                                to.format("%R"),
                                g.len(),
                            ))
                        } else {
                            Some(format!(
                                "{} - {} ({})",
                                from.format("%F"),
                                to.format("%F"),
                                g.len(),
                            ))
                        }
                    } else {
                        Some(format!(
                            "{} - {} ({})",
                            from.map(|d| format!("{}", d.format("%F %R")))
                                .unwrap_or_else(|| "-".to_string()),
                            to.map(|d| format!("{}", d.format("%F %R")))
                                .unwrap_or_else(|| "-".to_string()),
                            g.len(),
                        ))
                    }
                },
            }
        }
    }
}

impl<'a> From<&'a Photo> for PhotoLink {
    fn from(p: &'a Photo) -> PhotoLink {
        PhotoLink {
            title: None,
            href: format!("/img/{}", p.id),
            id: p.id,
            lable: p.date.map(|d| format!("{}", d.format("%F %T"))),
        }
    }
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    if let Some(pidfile) = args.value_of("PIDFILE") {
        handle_pid_file(pidfile, args.is_present("REPLACE")).unwrap()
    }

    let mut server = Nickel::new();
    server.utilize(RequestLoggerMiddleware);
    wrap3!(server.get "/static/",.. static_file);
    server.utilize(MemcacheMiddleware::new(vec![
        ("tcp://127.0.0.1:11211".into(), 1),
    ]));
    server.utilize(SessionMiddleware::new(&jwt_key()));
    let dm: DieselMiddleware<PgConnection> =
        DieselMiddleware::new(&dburl(), 5, Box::new(NopErrorHandler)).unwrap();
    server.utilize(dm);
    server.utilize(PhotosDirMiddleware::new(photos_dir()));

    wrap3!(server.get  "/login",         login);
    wrap3!(server.post "/login",         do_login);
    wrap3!(server.get  "/logout",        logout);
    wrap3!(server.get "/",               all_years);
    use self::admin::{rotate, set_grade, set_person, set_tag};
    wrap3!(server.get "/ac/tag",         auto_complete_tag);
    wrap3!(server.get "/ac/person",      auto_complete_person);
    wrap3!(server.post "/adm/grade",     set_grade);
    wrap3!(server.post "/adm/person",    set_person);
    wrap3!(server.post "/adm/rotate",    rotate);
    wrap3!(server.post "/adm/tag",       set_tag);
    wrap3!(server.get "/img/{}[-]{}\\.jpg", show_image: id, size);
    wrap3!(server.get "/img/{}",         photo_details: id);
    wrap3!(server.get "/next",           next_image);
    wrap3!(server.get "/prev",           prev_image);
    wrap3!(server.get "/tag/",           tag_all);
    wrap3!(server.get "/tag/{}",         tag_one: tag);
    wrap3!(server.get "/place/",         place_all);
    wrap3!(server.get "/place/{}",       place_one: slug);
    wrap3!(server.get "/person/",        person_all);
    wrap3!(server.get "/person/{}",      person_one: slug);
    wrap3!(server.get "/random",         random_image);
    wrap3!(server.get "/0/",             all_null_date);
    wrap3!(server.get "/{}/",            months_in_year: year);
    wrap3!(server.get "/{}/{}/",         days_in_month: year, month);
    wrap3!(server.get "/{}/{}/{}",       all_for_day: year, month, day);
    wrap3!(server.get "/thisday",        on_this_day);

    server.handle_error(
        custom_errors as fn(&mut NickelError, &mut Request) -> Action,
    );

    server
        .listen(&*env_or("RPHOTOS_LISTEN", "127.0.0.1:6767"))
        .map_err(|e| Error::Other(format!("listen: {}", e)))?;
    Ok(())
}

fn custom_errors(err: &mut NickelError, req: &mut Request) -> Action {
    if let Some(ref mut res) = err.stream {
        if res.status() == StatusCode::NotFound {
            templates::not_found(res, req).unwrap();
            return Halt(());
        }
    }

    Continue(())
}

fn login<'mw>(
    req: &mut Request,
    mut res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    res.clear_jwt();
    let next = sanitize_next(req.query().get("next")).map(String::from);
    res.ok(|o| templates::login(o, req, next, None))
}

fn do_login<'mw>(
    req: &mut Request,
    mut res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    let next = {
        let c: &PgConnection = &req.db_conn();
        let form_data = try_with!(res, req.form_body());
        let next = sanitize_next(form_data.get("next")).map(String::from);
        if let (Some(user), Some(pw)) =
            (form_data.get("user"), form_data.get("password"))
        {
            use schema::users::dsl::*;
            if let Ok(hash) = users
                .filter(username.eq(user))
                .select(password)
                .first::<String>(c)
            {
                debug!("Hash for {} is {}", user, hash);
                if djangohashers::check_password_tolerant(pw, &hash) {
                    info!("User {} logged in", user);
                    res.set_jwt_user(user);
                    return res.redirect(next.unwrap_or_else(|| "/".into()));
                }
                info!(
                    "Login failed: Password verification failed for {:?}",
                    user,
                );
            } else {
                info!("Login failed: No hash found for {:?}", user);
            }
        }
        next
    };
    let message = Some("Login failed, please try again");
    res.ok(|o| templates::login(o, req, next, message))
}

fn sanitize_next(next: Option<&str>) -> Option<&str> {
    if let Some(next) = next {
        use regex::Regex;
        let re = Regex::new(r"^/([a-z0-9._-]+/?)*$").unwrap();
        if re.is_match(next) {
            return Some(next);
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

fn logout<'mw>(
    _req: &mut Request,
    mut res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    res.clear_jwt();
    res.redirect("/")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeTag {
    Small,
    Medium,
    Large,
}
impl SizeTag {
    pub fn px(&self) -> u32 {
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

fn show_image<'mw>(
    req: &Request,
    mut res: Response<'mw>,
    the_id: i32,
    size: SizeTag,
) -> MiddlewareResult<'mw> {
    use schema::photos::dsl::photos;
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
                let data = get_image_data(req, &tphoto, size)
                    .expect("Get image data");
                res.set((MediaType::Jpeg, far_expires()));
                return res.send(data);
            }
        }
    }
    res.not_found("No such image")
}

fn get_image_data(
    req: &Request,
    photo: &Photo,
    size: SizeTag,
) -> Result<Vec<u8>, image::ImageError> {
    req.cached_or(&photo.cache_key(&size), || {
        let size = size.px();
        req.photos().scale_image(photo, size, size)
    })
}

fn tag_all<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    use schema::tags::dsl::{id, tag_name, tags};
    let c: &PgConnection = &req.db_conn();
    let query = tags.into_boxed();
    let query = if req.authorized_user().is_some() {
        query
    } else {
        use schema::photo_tags::dsl as tp;
        use schema::photos::dsl as p;
        query.filter(id.eq_any(tp::photo_tags.select(tp::tag_id).filter(
            tp::photo_id.eq_any(p::photos.select(p::id).filter(p::is_public)),
        )))
    };
    res.ok(|o| {
        templates::tags(
            o,
            req,
            &query.order(tag_name).load(c).expect("List tags"),
        )
    })
}

fn tag_one<'mw>(
    req: &mut Request,
    res: Response<'mw>,
    tslug: String,
) -> MiddlewareResult<'mw> {
    use schema::tags::dsl::{slug, tags};
    let c: &PgConnection = &req.db_conn();
    if let Ok(tag) = tags.filter(slug.eq(tslug)).first::<Tag>(c) {
        use schema::photo_tags::dsl::{photo_id, photo_tags, tag_id};
        use schema::photos::dsl::id;
        let photos = Photo::query(req.authorized_user().is_some()).filter(
            id.eq_any(photo_tags.select(photo_id).filter(tag_id.eq(tag.id))),
        );
        let (links, coords) = links_by_time(req, photos);
        return res.ok(|o| templates::tag(o, req, &links, &coords, &tag));
    }
    res.not_found("Not a tag")
}

fn place_all<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    use schema::places::dsl::{id, place_name, places};
    let query = places.into_boxed();
    let query = if req.authorized_user().is_some() {
        query
    } else {
        use schema::photo_places::dsl as pp;
        use schema::photos::dsl as p;
        query.filter(id.eq_any(pp::photo_places.select(pp::place_id).filter(
            pp::photo_id.eq_any(p::photos.select(p::id).filter(p::is_public)),
        )))
    };
    let c: &PgConnection = &req.db_conn();
    res.ok(|o| {
        templates::places(
            o,
            req,
            &query.order(place_name).load(c).expect("List places"),
        )
    })
}

fn static_file<'mw>(
    _req: &Request,
    mut res: Response<'mw>,
    path: &str,
) -> MiddlewareResult<'mw> {
    use templates::statics::StaticFile;
    if let Some(s) = StaticFile::get(path) {
        res.set((ContentType(s.mime()), far_expires()));
        return res.send(s.content);
    }
    res.not_found("No such file")
}

fn place_one<'mw>(
    req: &mut Request,
    res: Response<'mw>,
    tslug: String,
) -> MiddlewareResult<'mw> {
    use schema::places::dsl::{places, slug};
    let c: &PgConnection = &req.db_conn();
    if let Ok(place) = places.filter(slug.eq(tslug)).first::<Place>(c) {
        use schema::photo_places::dsl::{photo_id, photo_places, place_id};
        use schema::photos::dsl::id;
        let photos =
            Photo::query(req.authorized_user().is_some()).filter(id.eq_any(
                photo_places.select(photo_id).filter(place_id.eq(place.id)),
            ));
        let (links, coord) = links_by_time(req, photos);
        return res.ok(|o| templates::place(o, req, &links, &coord, &place));
    }
    res.not_found("Not a place")
}

fn person_all<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    use schema::people::dsl::{id, people, person_name};
    let query = people.into_boxed();
    let query = if req.authorized_user().is_some() {
        query
    } else {
        use schema::photo_people::dsl as pp;
        use schema::photos::dsl as p;
        query.filter(
            id.eq_any(
                pp::photo_people.select(pp::person_id).filter(
                    pp::photo_id
                        .eq_any(p::photos.select(p::id).filter(p::is_public)),
                ),
            ),
        )
    };
    let c: &PgConnection = &req.db_conn();
    res.ok(|o| {
        templates::people(
            o,
            req,
            &query.order(person_name).load(c).expect("list people"),
        )
    })
}

fn person_one<'mw>(
    req: &mut Request,
    res: Response<'mw>,
    tslug: String,
) -> MiddlewareResult<'mw> {
    use schema::people::dsl::{people, slug};
    let c: &PgConnection = &req.db_conn();
    if let Ok(person) = people.filter(slug.eq(tslug)).first::<Person>(c) {
        use schema::photo_people::dsl::{person_id, photo_id, photo_people};
        use schema::photos::dsl::id;
        let photos = Photo::query(req.authorized_user().is_some()).filter(
            id.eq_any(
                photo_people
                    .select(photo_id)
                    .filter(person_id.eq(person.id)),
            ),
        );
        let (links, coords) = links_by_time(req, photos);
        return res.ok(|o| templates::person(o, req, &links, &coords, &person));
    }
    res.not_found("Not a person")
}

fn random_image<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    use diesel::expression::dsl::sql;
    use diesel::sql_types::Integer;
    use schema::photos::dsl::id;
    let c: &PgConnection = &req.db_conn();
    let photo: i32 = Photo::query(req.authorized_user().is_some())
        .select(id)
        .limit(1)
        .order(sql::<Integer>("random()"))
        .first(c)
        .unwrap();
    info!("Random: {:?}", photo);
    res.redirect(format!("/img/{}", photo)) // to photo_details
}

fn photo_details<'mw>(
    req: &mut Request,
    res: Response<'mw>,
    id: i32,
) -> MiddlewareResult<'mw> {
    use schema::photos::dsl::photos;
    let c: &PgConnection = &req.db_conn();
    if let Ok(tphoto) = photos.find(id).first::<Photo>(c) {
        if req.authorized_user().is_some() || tphoto.is_public() {
            return res.ok(|o| {
                templates::details(
                    o,
                    req,
                    &tphoto
                        .date
                        .map(|d| {
                            vec![
                                Link::year(d.year()),
                                Link::month(d.year(), d.month()),
                                Link::day(d.year(), d.month(), d.day()),
                                Link::prev(tphoto.id),
                                Link::next(tphoto.id),
                            ]
                        })
                        .unwrap_or_default(),
                    &tphoto.load_people(c).unwrap(),
                    &tphoto.load_places(c).unwrap(),
                    &tphoto.load_tags(c).unwrap(),
                    &tphoto.load_position(c),
                    &tphoto.load_attribution(c),
                    &tphoto.load_camera(c),
                    &tphoto,
                )
            });
        }
    }
    res.not_found("Photo not found")
}

pub type Link = Html<String>;

impl Link {
    fn year(year: i32) -> Self {
        Html(format!(
            "<a href='/{0}/' title='Images from {0}' accessKey='y'>{0}</a>",
            year,
        ))
    }
    fn month(year: i32, month: u32) -> Self {
        Html(format!(
            "<a href='/{0}/{1}/' title='Images from {2} {0}' \
             accessKey='m'>{1}</a>",
            year,
            month,
            monthname(month),
        ))
    }
    fn day(year: i32, month: u32, day: u32) -> Self {
        Html(format!(
            "<a href='/{0}/{1}/{2}' title='Images from {2} {3} {0}' \
             accessKey='d'>{2}</a>",
            year,
            month,
            day,
            monthname(month),
        ))
    }
    fn prev(from: i32) -> Self {
        Html(format!(
            "<a href='/prev?from={}' title='Previous image (by time)'>\
             \u{2190}</a>",
            from,
        ))
    }
    fn next(from: i32) -> Self {
        Html(format!(
            "<a href='/next?from={}' title='Next image (by time)' \
             accessKey='n'>\u{2192}</a>",
            from,
        ))
    }
}

fn auto_complete_tag<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    if let Some(q) = req.query().get("q").map(String::from) {
        use schema::tags::dsl::{tag_name, tags};
        let c: &PgConnection = &req.db_conn();
        let q = tags.select(tag_name)
            .filter(tag_name.ilike(q + "%"))
            .order(tag_name)
            .limit(10);
        res.send(q.load::<String>(c).unwrap().to_json())
    } else {
        res.error(StatusCode::BadRequest, "Missing 'q' parameter")
    }
}

fn auto_complete_person<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    if let Some(q) = req.query().get("q").map(String::from) {
        use schema::people::dsl::{people, person_name};
        let c: &PgConnection = &req.db_conn();
        let q = people
            .select(person_name)
            .filter(person_name.ilike(q + "%"))
            .order(person_name)
            .limit(10);
        res.send(q.load::<String>(c).unwrap().to_json())
    } else {
        res.error(StatusCode::BadRequest, "Missing 'q' parameter")
    }
}
