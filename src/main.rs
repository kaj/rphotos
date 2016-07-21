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

use nickel_diesel::{DieselMiddleware, DieselRequestExtensions};
use r2d2::NopErrorHandler;
use chrono::Duration as ChDuration;
use chrono::Datelike;
use dotenv::dotenv;
use hyper::header::{Expires, HttpDate};
use nickel::{FormBody, HttpRouter, MediaType, MiddlewareResult, Nickel,
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

#[derive(Debug, Clone, RustcEncodable)]
struct Group {
    title: String,
    url: String,
    count: i64,
    photo: Photo,
}

#[derive(Debug, Clone, RustcEncodable)]
struct Coord {
    x: f64,
    y: f64,
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
    wrap2!(server.get /:year/,             months_in_year);
    wrap2!(server.get /:year/:month/,      days_in_month);
    wrap2!(server.get /:year/:month/:day/, all_for_day);
    wrap2!(server.get /thisday,            on_this_day);

    server.listen(&*env_or("RPHOTOS_LISTEN", "127.0.0.1:6767"));
}

fn login<'mw>(_req: &mut Request,
              mut res: Response<'mw>)
              -> MiddlewareResult<'mw> {
    res.clear_jwt_user();
    render!(res, "templates/login.tpl", {})
}

fn do_login<'mw>(req: &mut Request,
                 mut res: Response<'mw>)
                 -> MiddlewareResult<'mw> {
    let connection = req.db_conn();
    let c: &PgConnection = &connection;
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
    render!(res, "templates/login.tpl", {})
}

fn logout<'mw>(_req: &mut Request,
               mut res: Response<'mw>)
               -> MiddlewareResult<'mw> {
    res.clear_jwt_user();
    res.redirect("/")
}

enum SizeTag {
    Small,
    Medium,
    Large,
}
impl SizeTag {
    fn px(&self) -> u32 {
        match *self {
            SizeTag::Small => 200,
            SizeTag::Medium => 800,
            SizeTag::Large => 1200,
        }
    }
}

impl FromSlug for SizeTag {
    fn parse_slug(slug: &str) -> Option<Self> {
        match slug {
            "s" => Some(SizeTag::Small),
            "m" => Some(SizeTag::Medium),
            "l" => Some(SizeTag::Large),
            _ => None
        }
    }
}

fn opt<T: FromSlug>(req: &Request, name: &str) -> Option<T> {
    req.param(name).and_then(T::parse_slug)
}


fn show_image<'mw>(req: &Request,
                   mut res: Response<'mw>,
                   the_id: i32,
                   size: SizeTag)
                   -> MiddlewareResult<'mw> {
    use rphotos::schema::photos::dsl::*;
    let connection = req.db_conn();
    let c: &PgConnection = &connection;
    if let Ok(tphoto) = photos.find(the_id).first::<Photo>(c) {
        if req.authorized_user().is_some() || tphoto.is_public() {
            let size = size.px();
            match req.photos().get_scaled_image(tphoto, size, size) {
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

fn tag_all<'mw>(req: &mut Request,
                res: Response<'mw>)
                -> MiddlewareResult<'mw> {
    use rphotos::schema::tags::dsl::*;
    let connection = req.db_conn();
    let c: &PgConnection = &connection;
    return render!(res, "templates/tags.tpl", {
        user: Option<String> = req.authorized_user(),
        // TODO order by tag name!
        tags: Vec<Tag> = tags.load(c).unwrap()
    });
}

fn tag_one<'mw>(req: &mut Request,
                res: Response<'mw>,
                tslug: String)
                -> MiddlewareResult<'mw> {
    use rphotos::schema::tags::dsl::*;
    let connection = req.db_conn();
    let c: &PgConnection = &connection;
    if let Ok(tag) = tags.filter(slug.eq(tslug)).first::<Tag>(c) {
        use rphotos::schema::photos::dsl::*;
        use rphotos::schema::photo_tags::dsl::{photo_id, photo_tags, tag_id};
        return render!(res, "templates/tag.tpl", {
            user: Option<String> = req.authorized_user(),
            photos: Vec<Photo> = photos
                .filter(id.eq_any(photo_tags.select(photo_id).filter(tag_id.eq(tag.id))))
                .load(c).unwrap(),
            // TODO
            // .only_public(req.authorized_user().is_none())
            // .desc_nulls_last("grade")
            // .desc_nulls_last("date")
            tag: Tag = tag
        });
    }
    res.error(StatusCode::NotFound, "Not a tag")
}

fn place_all<'mw>(req: &mut Request,
                  res: Response<'mw>)
                  -> MiddlewareResult<'mw> {
    use rphotos::schema::places::dsl::*;
    let connection = req.db_conn();
    let c: &PgConnection = &connection;
    return render!(res, "templates/places.tpl", {
        user: Option<String> = req.authorized_user(),
        // TODO order by place name!
        places: Vec<Place> = places.load(c).unwrap()
    });
}

fn place_one<'mw>(req: &mut Request,
                  res: Response<'mw>,
                  tslug: String)
                  -> MiddlewareResult<'mw> {
    use rphotos::schema::places::dsl::*;
    let connection = req.db_conn();
    let c: &PgConnection = &connection;
    if let Ok(place) = places.filter(slug.eq(tslug)).first::<Place>(c) {
        use rphotos::schema::photos::dsl::*;
        use rphotos::schema::photo_places::dsl::{photo_id, photo_places, place_id};
        return render!(res, "templates/place.tpl", {
            user: Option<String> = req.authorized_user(),
            photos: Vec<Photo> = photos
                .filter(id.eq_any(photo_places.select(photo_id).filter(place_id.eq(place.id))))
                .load(c).unwrap(),
            // TODO
            // .only_public(req.authorized_user().is_none())
            // .desc_nulls_last("grade")
            // .desc_nulls_last("date")
            place: Place = place
        });
    }
    res.error(StatusCode::NotFound, "Not a place")
}

fn person_all<'mw>(req: &mut Request,
                   res: Response<'mw>)
                   -> MiddlewareResult<'mw> {
    use rphotos::schema::people::dsl::*;
    let connection = req.db_conn();
    let c: &PgConnection = &connection;
    return render!(res, "templates/people.tpl", {
        user: Option<String> = req.authorized_user(),
        // TODO order by name!
        people: Vec<Person> = people.load(c).expect("list people")
    });
}

fn person_one<'mw>(req: &mut Request,
                   res: Response<'mw>,
                   tslug: String)
                   -> MiddlewareResult<'mw> {
    use rphotos::schema::people::dsl::*;
    let connection = req.db_conn();
    let c: &PgConnection = &connection;
    if let Ok(person) = people.filter(slug.eq(tslug)).first::<Person>(c) {
        use rphotos::schema::photos::dsl::*;
        use rphotos::schema::photo_people::dsl::{photo_id, photo_people, person_id};
        return render!(res, "templates/person.tpl", {
            user: Option<String> = req.authorized_user(),
            photos: Vec<Photo> = photos
                .filter(id.eq_any(photo_people.select(photo_id)
                                              .filter(person_id.eq(person.id))))
                .load(c).unwrap(),
            // TODO
            // .only_public(req.authorized_user().is_none())
            // .desc_nulls_last("grade")
            // .desc_nulls_last("date")
            person: Person = person
        });
    }
    res.error(StatusCode::NotFound, "Not a person")
}

fn photo_details<'mw>(req: &mut Request,
                      res: Response<'mw>,
                      id: i32)
                      -> MiddlewareResult<'mw> {
    use rphotos::schema::photos::dsl::photos;
    let connection = req.db_conn();
    let c: &PgConnection = &connection;
    if let Ok(tphoto) = photos.find(id).first::<Photo>(c) {
        if req.authorized_user().is_some() || tphoto.is_public() {
            return render!(res, "templates/details.tpl", {
                user: Option<String> = req.authorized_user(),
                lpath: Vec<Link> =
                    tphoto.date
                    .map(|d| vec![Link::year(d.year()),
                                  Link::month(d.year(), d.month() as u8),
                                  Link::day(d.year(), d.month() as u8, d.day())])
                    .unwrap_or_else(|| vec![]),
                people: Vec<Person> = {
                    use rphotos::schema::people::dsl::{people, id};
                    use rphotos::schema::photo_people::dsl::{photo_people, photo_id, person_id};
                    people.filter(id.eq_any(photo_people.select(person_id)
                                            .filter(photo_id.eq(tphoto.id))))
                        .load(c).unwrap()
                },
                places: Vec<Place> = {
                    use rphotos::schema::places::dsl::{places, id};
                    use rphotos::schema::photo_places::dsl::{photo_places, photo_id, place_id};
                    places.filter(id.eq_any(photo_places.select(place_id)
                                            .filter(photo_id.eq(tphoto.id))))
                        .load(c).unwrap()
                },
                tags: Vec<Tag> = {
                    use rphotos::schema::tags::dsl::{tags, id};
                    use rphotos::schema::photo_tags::dsl::{photo_tags, photo_id, tag_id};
                    tags.filter(id.eq_any(photo_tags.select(tag_id)
                                          .filter(photo_id.eq(tphoto.id))))
                        .load(c).unwrap()
                },
                position: Option<Coord> = {
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
                time: String = match tphoto.date {
                    Some(d) => d.format("%T").to_string(),
                    None => "".to_string()
                },
                year: Option<i32> = tphoto.date.map(|d| d.year()),
                month: Option<u32> = tphoto.date.map(|d| d.month()),
                day: Option<u32> = tphoto.date.map(|d| d.day()),
                photo: Photo = tphoto
            });
        }
    }
    res.error(StatusCode::NotFound, "Photo not found")
}

fn all_years<'mw>(req: &mut Request,
                  res: Response<'mw>)
                  -> MiddlewareResult<'mw> {

    use rphotos::schema::photos::dsl::*;
    let connection = req.db_conn();
    let c: &PgConnection = &connection;

    return render!(res, "templates/groups.tpl", {
        user: Option<String> = req.authorized_user(),
        title: &'static str = "All photos",
        groups: Vec<Group> =
            // FIXME only public if not logged on!
            SqlLiteral::new(format!(concat!(
                "select extract(year from date) y, count(*) c",
                " from photos{} group by y order by y"),
                                    if req.authorized_user().is_none() {
                                        " where grade >= 4"
                                    } else {
                                        ""
                                    }))
            .load::<(Option<f64>, i64)>(c).unwrap()
            .iter().map(|&(year, count)| {
                let q = photos
                    // .only_public(req.authorized_user().is_none())
                    // .filter(path.like("%.JPG"))
                    .order((grade.desc(), date.asc()))
                    .limit(1);
                let photo =
                    if let Some(year) = year {
                        let year = year as i32;
                        q.filter(date.ge(NaiveDate::from_ymd(year, 1, 1)
                                         .and_hms(0, 0, 0)))
                         .filter(date.lt(NaiveDate::from_ymd(year + 1, 1, 1)
                                         .and_hms(0, 0, 0)))
                         .first::<Photo>(c).unwrap()
                    } else {
                        q.filter(date.is_null())
                         .first::<Photo>(c).unwrap()
                    };
                Group {
                    title: year.map(|y|format!("{}", y))
                               .unwrap_or("-".to_string()),
                    url: format!("/{}/", year.unwrap_or(0f64)),
                    count: count,
                    photo: photo
                }
            }).collect()
    });
}

fn months_in_year<'mw>(req: &mut Request,
                       res: Response<'mw>,
                       year: i32)
                       -> MiddlewareResult<'mw> {
    use rphotos::schema::photos::dsl::*;
    let connection = req.db_conn();
    let c: &PgConnection = &connection;
    
    render!(res, "templates/groups.tpl", {
        user: Option<String> = req.authorized_user(),
        title: String = format!("Photos from {}", year),
        groups: Vec<Group> =
            // FIXME only public if not logged on!
            SqlLiteral::new(format!(concat!(
                "select extract(month from date) m, count(*) c ",
                "from photos where extract(year from date)={}{} ",
                "group by m order by m"),
                                    year,
                                    if req.authorized_user().is_none() {
                                        " and grade >= 4"
                                    } else {
                                        ""
                                    }))
            .load::<(Option<f64>, i64)>(c).unwrap()
            .iter().map(|&(month, count)| {
                let month = month.map(|y| y as u32).unwrap_or(0);
                let fromdate = NaiveDate::from_ymd(year, month, 1).and_hms(0, 0, 0);
                let todate =
                    if month == 12 { NaiveDate::from_ymd(year + 1, 1, 1) }
                    else { NaiveDate::from_ymd(year, month + 1, 1) }
                    .and_hms(0, 0, 0);
                let photo = photos
                    // .only_public(req.authorized_user().is_none())
                    .filter(date.ge(fromdate))
                    .filter(date.lt(todate))
                    // .filter(path.like("%.JPG"))
                    .order((grade.desc(), date.asc()))
                    .limit(1)
                    .first::<Photo>(c).unwrap();

                Group {
                    title: monthname(month as u8).to_string(),
                    url: format!("/{}/{}/", year, month),
                    count: count,
                    photo: photo
                }
            }).collect()
    })
}

fn days_in_month<'mw>(req: &mut Request,
                      res: Response<'mw>,
                      year: i32,
                      month: u8)
                      -> MiddlewareResult<'mw> {
    use rphotos::schema::photos::dsl::*;
    let connection = req.db_conn();
    let c: &PgConnection = &connection;

    render!(res, "templates/groups.tpl", {
        user: Option<String> = req.authorized_user(),
        lpath: Vec<Link> = vec![Link::year(year)],
        title: String = format!("Photos from {} {}", monthname(month), year),
        groups: Vec<Group> =
            // FIXME only public if not logged on!
            SqlLiteral::new(format!(concat!(
                "select extract(day from date) d, count(*) c ",
                "from photos where extract(year from date)={} ",
                "and extract(month from date)={}{} group by d order by d"),
                                    year, month,
                                    if req.authorized_user().is_none() {
                                        " and grade >= 4"
                                    } else {
                                        ""
                                    }))
            .load::<(Option<f64>, i64)>(c).unwrap()
            .iter().map(|&(day, count)| {
                let day = day.map(|y| y as u32).unwrap_or(0);
                let fromdate = NaiveDate::from_ymd(year, month as u32, day)
                    .and_hms(0, 0, 0);
                let photo = photos
                    // .only_public(req.authorized_user().is_none())
                    .filter(date.ge(fromdate))
                    .filter(date.lt(fromdate + ChDuration::days(1)))
                    // .filter(path.like("%.JPG"))
                    .order((grade.desc(), date.asc()))
                    .limit(1)
                    .first::<Photo>(c).unwrap();

                Group {
                    title: format!("{}", day),
                    url: format!("/{}/{}/{}", year, month, day),
                    count: count,
                    photo: photo
                }
            }).collect()
    })
}

fn all_for_day<'mw>(req: &mut Request,
                    res: Response<'mw>,
                    year: i32,
                    month: u8,
                    day: u32)
                    -> MiddlewareResult<'mw> {
    let thedate = NaiveDate::from_ymd(year, month as u32, day)
        .and_hms(0, 0, 0);
    use rphotos::schema::photos::dsl::*;
    let pq = photos
        .filter(date.ge(thedate))
        .filter(date.lt(thedate + ChDuration::days(1)))
        // .filter(path.like("%.JPG"))
        .order((grade.desc(), date.asc()))
        .limit(500);

    let connection = req.db_conn();
    let c: &PgConnection = &connection;
    render!(res, "templates/index.tpl", {
        user: Option<String> = req.authorized_user(),
        lpath: Vec<Link> = vec![Link::year(year),
                                Link::month(year, month)],
        title: String = format!("Photos from {} {} {}",
                                day, monthname(month), year),
        photos: Vec<Photo> =
            if req.authorized_user().is_none() {
                pq.filter(grade.ge(&rphotos::models::MIN_PUBLIC_GRADE))
                    .load(c).unwrap()
            } else {
                pq.load(c).unwrap()
            }
    })
}

fn on_this_day<'mw>(req: &mut Request,
                    res: Response<'mw>)
                      -> MiddlewareResult<'mw> {
    use rphotos::schema::photos::dsl::*;
    let connection = req.db_conn();
    let c: &PgConnection = &connection;

    let (month, day) = {
        let now = time::now();
        (now.tm_mon as u8 + 1, now.tm_mday as u32)
    };
    render!(res, "templates/groups.tpl", {
        user: Option<String> = req.authorized_user(),
        title: String = format!("Photos from {} {}", day, monthname(month)),
        groups: Vec<Group> =
            SqlLiteral::new(format!(concat!(
                "select extract(year from date) y, count(*) c ",
                "from photos where extract(month from date)={} ",
                "and extract(day from date)={}{} group by y order by y desc"),
                                    month, day,
                                    if req.authorized_user().is_none() {
                                        " and grade >= 4"
                                    } else {
                                        ""
                                    }))
            .load::<(Option<f64>, i64)>(c).unwrap()
            .iter().map(|&(year, count)| {
                let year = year.map(|y| y as i32).unwrap_or(0);
                let fromdate = NaiveDate::from_ymd(year, month as u32, day).and_hms(0, 0, 0);
                let photo = photos
                    // .only_public(req.authorized_user().is_none())
                    .filter(date.ge(fromdate))
                    .filter(date.lt(fromdate + ChDuration::days(1)))
                    // .filter(path.like("%.JPG"))
                    .order((grade.desc(), date.asc()))
                    .limit(1)
                    .first::<Photo>(c).unwrap();

                Group {
                    title: format!("{}", year),
                    url: format!("/{}/{}/{}", year, month, day),
                    count: count,
                    photo: photo
                }
            }).collect()
    })
}

#[derive(Debug, Clone, RustcEncodable)]
struct Link {
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
    fn month(year: i32, month: u8) -> Self {
        Link {
            url: format!("/{}/{}/", year, month),
            name: format!("{}", month),
        }
    }
    fn day(year: i32, month: u8, day: u32) -> Self {
        Link {
            url: format!("/{}/{}/{}", year, month, day),
            name: format!("{}", day),
        }
    }
}
