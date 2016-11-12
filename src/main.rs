#[macro_use]
extern crate nickel;
#[macro_use]
extern crate log;
extern crate djangohashers;
extern crate env_logger;
extern crate nickel_jwt_session;
extern crate rustc_serialize;
extern crate typemap;
extern crate plugin;
extern crate image;
extern crate hyper;
extern crate time;
extern crate chrono;
extern crate rexif;
extern crate rphotos;
extern crate r2d2;
extern crate nickel_diesel;
#[macro_use]
extern crate diesel;
extern crate r2d2_diesel;
extern crate dotenv;
extern crate memcached;

use memcached::Client;
use memcached::proto::{Operation, MultiOperation, NoReplyOperation, CasOperation, ProtoType};
use nickel_diesel::{DieselMiddleware, DieselRequestExtensions};
use r2d2::NopErrorHandler;
use chrono::Duration as ChDuration;
use chrono::Datelike;
use dotenv::dotenv;
use hyper::header::{Expires, HttpDate};
use nickel::{FormBody, Halt, HttpRouter, MediaType, MiddlewareResult, Nickel,
             Request, Response, StaticFilesHandler};
use nickel::extensions::response::Redirect;
use nickel_jwt_session::{SessionMiddleware, SessionRequestExtensions,
                         SessionResponseExtensions};
use time::Duration;
use nickel::status::StatusCode;
use diesel::expression::sql_literal::SqlLiteral;
use diesel::prelude::*;
use diesel::pg::PgConnection;
use chrono::naive::date::NaiveDate;

use rphotos::models::{Person, Photo, Place, Tag};
use std::io::{self, Write};

mod env;
use env::{dburl, env_or, jwt_key, photos_dir};

mod photosdir;

mod requestloggermiddleware;
use requestloggermiddleware::RequestLoggerMiddleware;

mod photosdirmiddleware;
use photosdirmiddleware::{PhotosDirMiddleware, PhotosDirRequestExtensions};

#[macro_use]
mod nickelext;
use nickelext::FromSlug;

use templates::Html;
pub static CSSLINK: Html<&'static str> =
    Html(include!(concat!(env!("OUT_DIR"), "/stylelink")));

#[derive(Debug, Clone, RustcEncodable)]
pub struct Group {
    title: String,
    url: String,
    count: i64,
    photo: Photo,
}

#[derive(Debug, Clone, RustcEncodable)]
pub struct Coord {
    x: f64,
    y: f64,
}

fn monthname(n: u32) -> &'static str {
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
        _ => "non-month",
    }
}


fn main() {
    dotenv().ok();
    env_logger::init().unwrap();
    info!("Initalized logger");

    let mut server = Nickel::new();
    server.utilize(RequestLoggerMiddleware);
    server.utilize(SessionMiddleware::new(&jwt_key()));
    // TODO This is a "build" location, not an "install" location ...
    let staticdir = concat!(env!("OUT_DIR"), "/static/");
    info!("Serving static files from {}", staticdir);
    server.utilize(StaticFilesHandler::new(staticdir));
    let dm: DieselMiddleware<PgConnection> =
        DieselMiddleware::new(&dburl(), 5, Box::new(NopErrorHandler)).unwrap();
    server.utilize(dm);
    server.utilize(PhotosDirMiddleware::new(photos_dir()));

    wrap2!(server.get  /login,             login);
    wrap2!(server.post /login,             do_login);
    wrap2!(server.get  /logout,            logout);
    server.get("/",                        all_years);
    wrap2!(server.get /img/:id/:size,      show_image);
    wrap2!(server.get /tag/,               tag_all);
    wrap2!(server.get /tag/:tag,           tag_one);
    wrap2!(server.get /place/,             place_all);
    wrap2!(server.get /place/:slug,        place_one);
    wrap2!(server.get /person/,            person_all);
    wrap2!(server.get /person/:slug,       person_one);
    wrap2!(server.get /details/:id,        photo_details);
    server.get("/0/",                      all_null_date);
    wrap2!(server.get /:year/,             months_in_year);
    wrap2!(server.get /:year/:month/,      days_in_month);
    wrap2!(server.get /:year/:month/:day/, all_for_day);
    wrap2!(server.get /thisday,            on_this_day);

    server.listen(&*env_or("RPHOTOS_LISTEN", "127.0.0.1:6767")).expect("listen");
}

fn login<'mw>(_req: &mut Request,
              mut res: Response<'mw>)
              -> MiddlewareResult<'mw> {
    res.clear_jwt();
    render(res, |o| templates::login(o))
}

fn do_login<'mw>(req: &mut Request,
                 mut res: Response<'mw>)
                 -> MiddlewareResult<'mw> {
    let c: &PgConnection = &req.db_conn();
    let form_data = try_with!(res, req.form_body());
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
                return res.redirect("/");
            }
            debug!("Password verification failed");
        } else {
            debug!("No hash found for {}", user);
        }
    }
    render(res, |o| templates::login(o))
}

fn logout<'mw>(_req: &mut Request,
               mut res: Response<'mw>)
               -> MiddlewareResult<'mw> {
    res.clear_jwt();
    res.redirect("/")
}

#[derive(Debug)]
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
            match get_image_data(req, tphoto, size) {
                Ok(buf) => {
                    res.set(MediaType::Jpeg);
                    res.set(Expires(HttpDate(time::now() +
                                             Duration::days(14))));
                    return res.send(buf);
                }
                Err(err) => {
                    return res.error(StatusCode::InternalServerError,
                                     format!("{}", err));
                }
            }
        }
    }
    res.error(StatusCode::NotFound, "No such image")
}

fn get_image_data(req: &Request, photo: Photo, size: SizeTag)
                  -> Result<Vec<u8>, image::ImageError>
{
    cached_or(&format!("rp{}{:?}", photo.id, size), || {
        let size = size.px();
        req.photos().get_scaled_image(photo, size, size)
    })
}

fn cached_or<F>(key: &str, init: F) -> Result<Vec<u8>, image::ImageError>
    where F: FnOnce() -> Result<Vec<u8>, image::ImageError>
{
    debug!("Cache key is {}", key);
    let servers = [
        ("tcp://127.0.0.1:11211", 1),
        ];

    match Client::connect(&servers, ProtoType::Binary) {
        Ok(mut client) => {
            match client.get(&key.as_bytes()) {
                Ok((data, _flags)) => {
                    debug!("Cached image {} found", key);
                    return Ok(data);
                },
                Err(err) => {
                    warn!("Error fetching {} from memcached: {:?}", key, err);
                }
            }

            let data = try!(init());
            let r = client.set(key.as_bytes(), &data, 0, 7 * 24 * 60 * 60);
            info!("Stored {} to memcached: {:?}", key, r);
            Ok(data)
        }
        Err(err) => {
            warn!("Error connecting to memcached: {}", err);
            init()
        }
    }
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
    render(res, |o| templates::tags(
        o,
        req.authorized_user(),
        query
            .order(tag_name)
            .load(c)
            .expect("List tags")))
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
        return render(res, |o| templates::tag(
            o,
            req.authorized_user(),
            Photo::query(req.authorized_user().is_some())
                .filter(id.eq_any(photo_tags.select(photo_id)
                                            .filter(tag_id.eq(tag.id))))
                .order((grade.desc().nulls_last(), date.desc().nulls_last()))
                .load(c).unwrap(),
            tag));
    }
    res.error(StatusCode::NotFound, "Not a tag")
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
    render(res, |o| templates::places(
        o,
        req.authorized_user(),
        query.order(place_name).load(c).expect("List places")))
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
        return render(res, |o| templates::place(
            o,
            req.authorized_user(),
            Photo::query(req.authorized_user().is_some())
                .filter(id.eq_any(photo_places.select(photo_id)
                                              .filter(place_id.eq(place.id))))
                .order((grade.desc().nulls_last(), date.desc().nulls_last()))
                .load(c).unwrap(),
            place));
    }
    res.error(StatusCode::NotFound, "Not a place")
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
    render(res, |o| templates::people(
        o,
        req.authorized_user(),
        query.order(person_name).load(c).expect("list people")))
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
        return render(res, |o| templates::person(
            o,
            req.authorized_user(),
            Photo::query(req.authorized_user().is_some())
                .filter(id.eq_any(photo_people.select(photo_id)
                                              .filter(person_id.eq(person.id))))
                .order((grade.desc().nulls_last(), date.desc().nulls_last()))
                .load(c).unwrap(),
            person));
    }
    res.error(StatusCode::NotFound, "Not a person")
}

fn photo_details<'mw>(req: &mut Request,
                      res: Response<'mw>,
                      id: i32)
                      -> MiddlewareResult<'mw> {
    use rphotos::schema::photos::dsl::photos;
    let c: &PgConnection = &req.db_conn();
    if let Ok(tphoto) = photos.find(id).first::<Photo>(c) {
        if req.authorized_user().is_some() || tphoto.is_public() {
            return render(res, |o| templates::details(
                o,
                tphoto.date
                    .map(|d| vec![Link::year(d.year()),
                                  Link::month(d.year(), d.month()),
                                  Link::day(d.year(), d.month(), d.day())])
                    .unwrap_or_else(|| vec![]),
                req.authorized_user(),
                {
                    use rphotos::schema::people::dsl::{people, id};
                    use rphotos::schema::photo_people::dsl::{photo_people, photo_id, person_id};
                    people.filter(id.eq_any(photo_people.select(person_id)
                                            .filter(photo_id.eq(tphoto.id))))
                        .load(c).unwrap()
                },
                {
                    use rphotos::schema::places::dsl::{places, id};
                    use rphotos::schema::photo_places::dsl::{photo_places, photo_id, place_id};
                    places.filter(id.eq_any(photo_places.select(place_id)
                                            .filter(photo_id.eq(tphoto.id))))
                        .load(c).unwrap()
                },
                {
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
                    use rphotos::schema::cameras::dsl::*;
                    tphoto.camera_id.map(|i| {
                        cameras.find(i).first(c).unwrap()
                    })
                },
                match tphoto.date {
                    Some(d) => d.format("%T").to_string(),
                    None => "".to_string()
                },
                tphoto));
        }
    }
    res.error(StatusCode::NotFound, "Photo not found")
}

fn all_years<'mw>(req: &mut Request,
                  res: Response<'mw>)
                  -> MiddlewareResult<'mw> {

    use rphotos::schema::photos::dsl::{date, grade};
    let c: &PgConnection = &req.db_conn();

    let user: Option<String> = req.authorized_user();
    let groups: Vec<Group> =
            SqlLiteral::new(format!(
                "select cast(extract(year from date) as int) y, count(*) c \
                 from photos{} group by y order by y desc nulls last",
                if req.authorized_user().is_none() {
                    " where is_public"
                } else {
                    ""
                }))
            .load::<(Option<i32>, i64)>(c).unwrap()
            .iter().map(|&(year, count)| {
                let q = Photo::query(req.authorized_user().is_some())
                    .order((grade.desc().nulls_last(), date.asc()))
                    .limit(1);
                let photo =
                    if let Some(year) = year {
                        q.filter(date.ge(NaiveDate::from_ymd(year, 1, 1)
                                         .and_hms(0, 0, 0)))
                         .filter(date.lt(NaiveDate::from_ymd(year + 1, 1, 1)
                                         .and_hms(0, 0, 0)))
                    } else {
                        q.filter(date.is_null())
                    };
                Group {
                    title: year.map(|y|format!("{}", y))
                               .unwrap_or("-".to_string()),
                    url: format!("/{}/", year.unwrap_or(0)),
                    count: count,
                    photo: photo.first::<Photo>(c).unwrap()
                }
            }).collect();

    render(res,
           |o| templates::groups(o, "All photos", Vec::new(), user, groups))
}

fn months_in_year<'mw>(req: &mut Request,
                       res: Response<'mw>,
                       year: i32)
                       -> MiddlewareResult<'mw> {
    use rphotos::schema::photos::dsl::{date, grade};
    let c: &PgConnection = &req.db_conn();

    let user: Option<String> = req.authorized_user();
    let title: String = format!("Photos from {}", year);
    let groups: Vec<Group> =
            SqlLiteral::new(format!(
                "select cast(extract(month from date) as int) m, count(*) c \
                 from photos where extract(year from date)={}{} \
                 group by m order by m",
                year,
                if req.authorized_user().is_none() {
                    " and is_public"
                } else {
                    ""
                }))
            .load::<(Option<i32>, i64)>(c).unwrap()
            .iter().map(|&(month, count)| {
                let month = month.map(|y| y as u32).unwrap_or(0);
                let fromdate = NaiveDate::from_ymd(year, month, 1).and_hms(0, 0, 0);
                let todate =
                    if month == 12 { NaiveDate::from_ymd(year + 1, 1, 1) }
                    else { NaiveDate::from_ymd(year, month + 1, 1) }
                    .and_hms(0, 0, 0);
                let photo = Photo::query(req.authorized_user().is_some())
                    .filter(date.ge(fromdate))
                    .filter(date.lt(todate))
                    .order((grade.desc().nulls_last(), date.asc()))
                    .limit(1)
                    .first::<Photo>(c).unwrap();

                Group {
                    title: monthname(month).to_string(),
                    url: format!("/{}/{}/", year, month),
                    count: count,
                    photo: photo
                }
            }).collect();

    if groups.is_empty() {
        res.error(StatusCode::NotFound, "No such image")
    } else {
        render(res, |o| templates::groups(o, &title, Vec::new(), user, groups))
    }
}

fn days_in_month<'mw>(req: &mut Request,
                      res: Response<'mw>,
                      year: i32,
                      month: u32)
                      -> MiddlewareResult<'mw> {
    use rphotos::schema::photos::dsl::{date, grade};
    let c: &PgConnection = &req.db_conn();

    let user: Option<String> = req.authorized_user();
    let lpath: Vec<Link> = vec![Link::year(year)];
    let title: String = format!("Photos from {} {}", monthname(month), year);
    let groups: Vec<Group> =
            SqlLiteral::new(format!(
                "select cast(extract(day from date) as int) d, count(*) c \
                 from photos where extract(year from date)={} \
                 and extract(month from date)={}{} group by d order by d",
                year, month,
                if req.authorized_user().is_none() {
                    " and is_public"
                } else {
                    ""
                }))
            .load::<(Option<i32>, i64)>(c).unwrap()
            .iter().map(|&(day, count)| {
                let day = day.map(|y| y as u32).unwrap_or(0);
                let fromdate = NaiveDate::from_ymd(year, month, day)
                    .and_hms(0, 0, 0);
                let photo = Photo::query(req.authorized_user().is_some())
                    .filter(date.ge(fromdate))
                    .filter(date.lt(fromdate + ChDuration::days(1)))
                    .order((grade.desc().nulls_last(), date.asc()))
                    .limit(1)
                    .first::<Photo>(c).unwrap();

                Group {
                    title: format!("{}", day),
                    url: format!("/{}/{}/{}", year, month, day),
                    count: count,
                    photo: photo
                }
            }).collect();

    if groups.is_empty() {
        res.error(StatusCode::NotFound, "No such image")
    } else {
        render(res, |o| templates::groups(o, &title, lpath, user, groups))
    }
}

fn all_null_date<'mw>(req: &mut Request,
                      res: Response<'mw>)
                      -> MiddlewareResult<'mw> {
    use rphotos::schema::photos::dsl::{date, path};

    let c: &PgConnection = &req.db_conn();
    render(res, |o| templates::index(
        o,
        &"Photos without a date",
        vec![],
        req.authorized_user(),
        Photo::query(req.authorized_user().is_some())
            .filter(date.is_null())
            .order(path.asc())
            .limit(500)
            .load(c).unwrap()))
}

fn all_for_day<'mw>(req: &mut Request,
                    res: Response<'mw>,
                    year: i32,
                    month: u32,
                    day: u32)
                    -> MiddlewareResult<'mw> {
    let thedate = NaiveDate::from_ymd(year, month, day).and_hms(0, 0, 0);
    use rphotos::schema::photos::dsl::{date, grade};

    let c: &PgConnection = &req.db_conn();

    let photos: Vec<Photo> = Photo::query(req.authorized_user().is_some())
            .filter(date.ge(thedate))
            .filter(date.lt(thedate + ChDuration::days(1)))
            .order((grade.desc().nulls_last(), date.asc()))
            .limit(500)
            .load(c).unwrap();

    if photos.is_empty() {
        res.error(StatusCode::NotFound, "No such image")
    } else {
        render(res, |o| templates::index(
            o,
            &format!("Photos from {} {} {}", day, monthname(month), year),
            vec![Link::year(year), Link::month(year, month)],
            req.authorized_user(),
            photos))
    }
}

fn on_this_day<'mw>(req: &mut Request,
                    res: Response<'mw>)
                    -> MiddlewareResult<'mw> {
    use rphotos::schema::photos::dsl::{date, grade};
    let c: &PgConnection = &req.db_conn();

    let (month, day) = {
        let now = time::now();
        (now.tm_mon as u32 + 1, now.tm_mday as u32)
    };
    render(res, |o| templates::groups(
        o,
        &format!("Photos from {} {}", day, monthname(month)),
        vec![],
        req.authorized_user(),
        SqlLiteral::new(format!(
                "select extract(year from date) y, count(*) c \
                 from photos where extract(month from date)={} \
                 and extract(day from date)={}{} group by y order by y desc",
                month, day,
                if req.authorized_user().is_none() {
                    " and is_public"
                } else {
                    ""
                }))
            .load::<(Option<f64>, i64)>(c).unwrap()
            .iter().map(|&(year, count)| {
                let year = year.map(|y| y as i32).unwrap_or(0);
                let fromdate = NaiveDate::from_ymd(year, month as u32, day)
                    .and_hms(0, 0, 0);
                let photo = Photo::query(req.authorized_user().is_some())
                    .filter(date.ge(fromdate))
                    .filter(date.lt(fromdate + ChDuration::days(1)))
                    .order((grade.desc().nulls_last(), date.asc()))
                    .limit(1)
                    .first::<Photo>(c).unwrap();

                Group {
                    title: format!("{}", year),
                    url: format!("/{}/{}/{}", year, month, day),
                    count: count,
                    photo: photo
                }
            }).collect()))
}

#[derive(Debug, Clone, RustcEncodable)]
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

fn render<'mw, F>(res: Response<'mw>, do_render: F)
                     ->MiddlewareResult<'mw>
    where F: FnOnce(&mut Write) -> io::Result<()>
{
    let mut stream = try!(res.start());
    match do_render(&mut stream) {
        Ok(()) => Ok(Halt(stream)),
        Err(e) => stream.bail(format!("Problem rendering template: {:?}", e))
    }
}

include!(concat!(env!("OUT_DIR"), "/templates.rs"));
