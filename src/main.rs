#[macro_use]
extern crate nickel;
#[macro_use]
extern crate log;
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

use nickel_diesel::{DieselMiddleware, DieselRequestExtensions};
use r2d2::NopErrorHandler;
use chrono::Duration as ChDuration;
use chrono::Datelike;
use hyper::header::{Expires, HttpDate};
use nickel::{FormBody, HttpRouter, MediaType, MiddlewareResult, Nickel,
             Request, Response, StaticFilesHandler};
use nickel::extensions::response::Redirect;
use nickel_jwt_session::{SessionMiddleware, SessionRequestExtensions,
                         SessionResponseExtensions};
use plugin::Pluggable;
use rustc_serialize::Encodable;
use time::Duration;
use nickel::status::StatusCode;
use diesel::expression::sql_literal::SqlLiteral;
use diesel::prelude::*;
use diesel::pg::PgConnection;
use chrono::naive::date::NaiveDate;

//mod models;
use rphotos::models::{Person, Photo, Place, Tag};

mod env;
use env::{dburl, env_or, jwt_key, photos_dir};

mod photosdir;

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
        _ => "non-month",
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
    let dm : DieselMiddleware<PgConnection> = DieselMiddleware::new(&dburl(),
                                         5,
                                         Box::new(NopErrorHandler)).unwrap();
    server.utilize(dm);
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

fn login<'mw>(_req: &mut Request,
              mut res: Response<'mw>)
              -> MiddlewareResult<'mw> {
    res.clear_jwt_user();
    render!(res, "templates/login.tpl", {})
}

fn do_login<'mw>(req: &mut Request,
                 mut res: Response<'mw>)
                 -> MiddlewareResult<'mw> {
    let form_data = try_with!(res, req.form_body());
    if let (Some(user), Some(password)) = (form_data.get("user"),
                                           form_data.get("password")) {
        // TODO Actual password hashing and checking
        if user == "kaj" && password == "kaj123" {
            res.set_jwt_user(user);
            return res.redirect("/");
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

fn show_image<'mw>(req: &mut Request,
                   mut res: Response<'mw>)
                   -> MiddlewareResult<'mw> {
    if let Ok(the_id) = req.param("id").unwrap().parse::<i32>() {
        use rphotos::schema::photos::dsl::*;
        let connection = req.db_conn();
        let c : &PgConnection = &connection;
        if let Ok(tphoto) = photos.find(the_id).first::<Photo>(c) {
            if req.authorized_user().is_some() || tphoto.is_public() {
                if let Some(size) = match req.param("size").unwrap() {
                    "s" => Some(200),
                    "m" => Some(800),
                    "l" => Some(1200),
                    _ => None,
                } {
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
        }
    }
    res.error(StatusCode::NotFound, "No such image")
}

fn tag_all<'mw>(req: &mut Request,
                res: Response<'mw>)
                -> MiddlewareResult<'mw> {
    /*
    use rphotos::schema::tag::dsl::*;
    let connection = req.db_conn();
    let c : &PgConnection = &connection;
    */
    return render!(res, "templates/tags.tpl", {
        user: Option<String> = req.authorized_user()
        // TODO order by tag name!
        // tags: Vec<Tag> = tag.load(c).unwrap()
    });
}
fn tag_one<'mw>(req: &mut Request,
                res: Response<'mw>)
                -> MiddlewareResult<'mw> {
    /*
    use rphotos::schema::tag::dsl::*;
    let tslug = req.param("tag").unwrap();
    let connection = req.db_conn();
    let c : &PgConnection = &connection;
    if let Ok(ttag) = tag.filter(slug.eq(tslug)).first::<Tag>(c) {
        return render!(res, "templates/tag.tpl", {
            user: Option<String> = req.authorized_user(),
            photos: Vec<i32> = vec![], / * FIXME Vec<Photo>
                orm_get_related::<Photo, Tag>(&tag, "photo_tag")
                .only_public(req.authorized_user().is_none())
                .desc_nulls_last("grade")
                .desc_nulls_last("date")
                .collect(req.db_conn()).unwrap(),* /
            tag: Tag = ttag
        });
    }
    */
    res.error(StatusCode::NotFound, "Not a tag")
}

fn place_all<'mw>(req: &mut Request,
                  res: Response<'mw>)
                  -> MiddlewareResult<'mw> {
    /*
    use rphotos::schema::place::dsl::*;
    let connection = req.db_conn();
    let c : &PgConnection = &connection;
    */
    return render!(res, "templates/places.tpl", {
        user: Option<String> = req.authorized_user()
        // TODO order by place name!
        // places: Vec<Place> = place.load(c).unwrap()
    });
}
fn place_one<'mw>(req: &mut Request,
                  res: Response<'mw>)
                  -> MiddlewareResult<'mw> {
    /*
    let tslug = req.param("slug").unwrap();
    use rphotos::schema::place::dsl::*;
    let connection = req.db_conn();
    let c : &PgConnection = &connection;
    if let Ok(tplace) = place.filter(slug.eq(tslug)).first::<Place>(c) {
        return render!(res, "templates/place.tpl", {
            user: Option<String> = req.authorized_user(),
            photos: Vec<i32> = vec![], / * TODO Vec<Photo> =
                orm_get_related::<Photo, Place>(&place, "photo_place")
                .only_public(req.authorized_user().is_none())
                .desc_nulls_last("grade")
                .desc_nulls_last("date")
                .collect(req.db_conn()).unwrap(), * /
            place: Place = tplace
        });
    }
    */
    res.error(StatusCode::NotFound, "Not a place")
}

fn person_all<'mw>(req: &mut Request,
                   res: Response<'mw>)
                   -> MiddlewareResult<'mw> {
    /*
    use rphotos::schema::person::dsl::*;
    let connection = req.db_conn();
    let c : &PgConnection = &connection;
    */
    return render!(res, "templates/people.tpl", {
        user: Option<String> = req.authorized_user()
        // TODO order by name!
        //people: Vec<Person> = person.load(c).expect("list persons")
    });
}
fn person_one<'mw>(req: &mut Request,
                   res: Response<'mw>)
                   -> MiddlewareResult<'mw> {
    /*
    let tslug = req.param("slug").unwrap();
    use rphotos::schema::person::dsl::*;
    let connection = req.db_conn();
    let c : &PgConnection = &connection;
    if let Ok(tperson) = person.filter(slug.eq(tslug)).first::<Person>(c) {
        return render!(res, "templates/person.tpl", {
            user: Option<String> = req.authorized_user(),
            photos: Vec<i32> = vec![], / * TODO Vec<Photo> =
                orm_get_related::<Photo, Person>(&person, "photo_person")
                .only_public(req.authorized_user().is_none())
                .desc_nulls_last("grade")
                .desc_nulls_last("date")
                .collect(req.db_conn()).unwrap(), * /
            person: Person = tperson
        });
    }
    */
    res.error(StatusCode::NotFound, "Not a place")
}

fn photo_details<'mw>(req: &mut Request,
                      res: Response<'mw>)
                      -> MiddlewareResult<'mw> {
    if let Ok(the_id) = req.param("id").unwrap().parse::<i32>() {
        use rphotos::schema::photos::dsl::*;
        let connection = req.db_conn();
        let c : &PgConnection = &connection;
        if let Ok(tphoto) = photos.find(the_id).first::<Photo>(c) {
            if req.authorized_user().is_some() || tphoto.is_public() {
                return render!(res, "templates/details.tpl", {
                    user: Option<String> = req.authorized_user(),
                    lpath: Vec<Link> =
                        tphoto.date
                        .map(|d| vec![Link::year(d.year()),
                                      Link::month(d.year(), d.month() as u8),
                                      Link::day(d.year(), d.month() as u8, d.day())])
                        .unwrap_or_else(|| vec![]),
                people: Vec<Person> = vec![],
                    // req.orm_get_related(&photo, "photo_person").unwrap(),
                places: Vec<Place> = vec![],
                    // req.orm_get_related(&photo, "photo_place").unwrap(),
                tags: Vec<Tag> = vec![],
                    // req.orm_get_related(&photo, "photo_tag").unwrap(),
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
    }
    res.error(StatusCode::NotFound, "Photo not found")
}

fn all_years<'mw>(req: &mut Request,
                  res: Response<'mw>)
                  -> MiddlewareResult<'mw> {

    use rphotos::schema::photos::dsl::*;
    let connection = req.db_conn();
    let c : &PgConnection = &connection;

    return render!(res, "templates/groups.tpl", {
        user: Option<String> = req.authorized_user(),
        title: &'static str = "All photos",
        groups: Vec<Group> =
            // FIXME only public if not logged on!
            SqlLiteral::new(concat!(
                "select extract(year from date) y, count(*) c",
                " from photos group by y order by y").to_string())
            .load::<(Option<f64>, i64)>(c).unwrap()
            .iter().map(|&(year, count)| {
                let q = photos
                    // .only_public(req.authorized_user().is_none())
                    // .filter(path.like("%.JPG"))
                    .order(date)
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
                       res: Response<'mw>)
                       -> MiddlewareResult<'mw> {
    use rphotos::schema::photos::dsl::*;
    let connection = req.db_conn();
    let c : &PgConnection = &connection;

    if let Ok(year) = req.param("year").unwrap().parse::<i32>() {
        return render!(res, "templates/groups.tpl", {
            user: Option<String> = req.authorized_user(),
            title: String = format!("Photos from {}", year),
            groups: Vec<Group> =
                // FIXME only public if not logged on!
                SqlLiteral::new(format!(concat!(
                    "select extract(month from date) m, count(*) c ",
                    "from photos where extract(year from date)={} group by m order by m"),
                                        year))
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
                        .order(date)
                        .limit(1)
                        .first::<Photo>(c).unwrap();

                    Group {
                        title: monthname(month as u8).to_string(),
                        url: format!("/{}/{}/", year, month),
                        count: count,
                        photo: photo
                    }
                }).collect()
        });
    }
    res.error(StatusCode::NotFound, "Not a year")
}

fn days_in_month<'mw>(req: &mut Request,
                      res: Response<'mw>)
                      -> MiddlewareResult<'mw> {
    use rphotos::schema::photos::dsl::*;
    let connection = req.db_conn();
    let c : &PgConnection = &connection;

    if let Ok(year) = req.param("year").unwrap().parse::<i32>() {
        if let Ok(month) = req.param("month").unwrap().parse::<u8>() {
            return render!(res, "templates/groups.tpl", {
                user: Option<String> = req.authorized_user(),
                lpath: Vec<Link> = vec![Link::year(year)],
                title: String = format!("Photos from {} {}", monthname(month),
                                        year),
                groups: Vec<Group> =
                // FIXME only public if not logged on!
                SqlLiteral::new(format!(concat!(
                    "select extract(day from date) d, count(*) c ",
                    "from photos where extract(year from date)={} ",
                    "and extract(month from date)={} group by d order by d"),
                                        year, month))
                .load::<(Option<f64>, i64)>(c).unwrap()
                    .iter().map(|&(day, count)| {
                        let day = day.map(|y| y as u32).unwrap_or(0);
                        let fromdate = NaiveDate::from_ymd(year, month as u32, day).and_hms(0, 0, 0);
                        let photo = photos
                            // .only_public(req.authorized_user().is_none())
                            .filter(date.ge(fromdate))
                            .filter(date.lt(fromdate + ChDuration::days(1)))
                            // .filter(path.like("%.JPG"))
                            .order(date)
                            .limit(1)
                            .first::<Photo>(c).unwrap();

                        Group {
                            title: format!("{}", day),
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

fn all_for_day<'mw>(req: &mut Request,
                    res: Response<'mw>)
                    -> MiddlewareResult<'mw> {
    if let Ok(year) = req.param("year").unwrap().parse::<i32>() {
        if let Ok(month) = req.param("month").unwrap().parse::<u8>() {
            if let Ok(day) = req.param("day").unwrap().parse::<u32>() {
                let thedate = NaiveDate::from_ymd(year, month as u32, day).and_hms(0, 0, 0);
                use rphotos::schema::photos::dsl::*;
                let pq = photos
                    .filter(date.ge(thedate))
                    .filter(date.lt(thedate + ChDuration::days(1)))
                    //.filter(path.like("%.JPG"))
                    .order(date)
                    .limit(500);
                /*
                let pq = if req.authorized_user().is_none() {
                    pq.filter(grade.ge(&rphotos::models::MIN_PUBLIC_GRADE))
                } else {
                    pq
            }*/
                /*
                        .no_raw()
               */
                let connection = req.db_conn();
                let c : &PgConnection = &connection;
                return render!(res, "templates/index.tpl", {
                    user: Option<String> = req.authorized_user(),
                    lpath: Vec<Link> = vec![Link::year(year),
                                            Link::month(year, month)],
                    title: String = format!("Photos from {} {} {}",
                                            day, monthname(month), year),
                    photos: Vec<Photo> = pq.load(c).unwrap()
                });
            }
        }
    }
    res.error(StatusCode::NotFound, "Not a day")
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
