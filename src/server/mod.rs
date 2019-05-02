#[macro_use]
mod admin;
mod context;
mod render_ructe;
mod splitlist;
mod views_by_date;

use self::context::create_session_filter;
pub use self::context::Context;
use self::render_ructe::RenderRucte;
use self::splitlist::*;
use self::views_by_date::*;
use crate::adm::result::Error;
use crate::env::{dburl, env_or, jwt_key};
use crate::models::{Person, Photo, Place, Tag};
use crate::pidfiles::handle_pid_file;
use crate::templates::{self, Html};
use chrono::Datelike;
use clap::ArgMatches;
use diesel::prelude::*;
use djangohashers;
use image;
use log::info;
use mime;
use serde::Deserialize;
use std::net::SocketAddr;
use warp::filters::path::Tail;
use warp::http::{header, Response, StatusCode};
use warp::{self, reply, Filter, Rejection, Reply};

pub struct PhotoLink {
    pub title: Option<String>,
    pub href: String,
    pub id: i32,
    // Size should not be optional, but make it best-effort for now.
    pub size: Option<(u32, u32)>,
    pub lable: Option<String>,
}

impl PhotoLink {
    fn for_group(g: &[Photo], base_url: &str, with_date: bool) -> PhotoLink {
        if g.len() == 1 {
            if with_date {
                PhotoLink::date_title(&g[0])
            } else {
                PhotoLink::no_title(&g[0])
            }
        } else {
            fn imgscore(p: &Photo) -> i16 {
                // Only score below 19 is worse than ungraded.
                p.grade.unwrap_or(19) * if p.is_public { 5 } else { 4 }
            }
            let photo = g.iter().max_by_key(|p| imgscore(p)).unwrap();
            let (title, lable) = {
                let from = g.last().and_then(|p| p.date);
                let to = g.first().and_then(|p| p.date);
                if let (Some(from), Some(to)) = (from, to) {
                    if from.date() == to.date() {
                        (
                            Some(from.format("%F").to_string()),
                            format!(
                                "{} - {} ({})",
                                from.format("%R"),
                                to.format("%R"),
                                g.len(),
                            ),
                        )
                    } else if from.year() == to.year() {
                        if from.month() == to.month() {
                            (
                                Some(from.format("%Y-%m").to_string()),
                                format!(
                                    "{} - {} ({})",
                                    from.format("%F"),
                                    to.format("%d"),
                                    g.len(),
                                ),
                            )
                        } else {
                            (
                                Some(from.format("%Y").to_string()),
                                format!(
                                    "{} - {} ({})",
                                    from.format("%F"),
                                    to.format("%m-%d"),
                                    g.len(),
                                ),
                            )
                        }
                    } else {
                        (
                            None,
                            format!(
                                "{} - {} ({})",
                                from.format("%F"),
                                to.format("%F"),
                                g.len(),
                            ),
                        )
                    }
                } else {
                    (
                        None,
                        format!(
                            "{} - {} ({})",
                            from.map(|d| format!("{}", d.format("%F %R")))
                                .unwrap_or_else(|| "-".to_string()),
                            to.map(|d| format!("{}", d.format("%F %R")))
                                .unwrap_or_else(|| "-".to_string()),
                            g.len(),
                        ),
                    )
                }
            };
            let title = if with_date { title } else { None };
            PhotoLink {
                title,
                href: format!(
                    "{}?from={}&to={}",
                    base_url,
                    g.last().map(|p| p.id).unwrap_or(0),
                    g.first().map(|p| p.id).unwrap_or(0),
                ),
                id: photo.id,
                size: photo.get_size(SizeTag::Small.px()),
                lable: Some(lable),
            }
        }
    }
    fn date_title(p: &Photo) -> PhotoLink {
        PhotoLink {
            title: p.date.map(|d| d.format("%F").to_string()),
            href: format!("/img/{}", p.id),
            id: p.id,
            size: p.get_size(SizeTag::Small.px()),
            lable: p.date.map(|d| d.format("%T").to_string()),
        }
    }
    fn no_title(p: &Photo) -> PhotoLink {
        PhotoLink {
            title: None, // p.date.map(|d| d.format("%F").to_string()),
            href: format!("/img/{}", p.id),
            id: p.id,
            size: p.get_size(SizeTag::Small.px()),
            lable: p.date.map(|d| d.format("%T").to_string()),
        }
    }
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    if let Some(pidfile) = args.value_of("PIDFILE") {
        handle_pid_file(pidfile, args.is_present("REPLACE")).unwrap()
    }
    let session_filter = create_session_filter(
        &dburl(),
        &env_or("MEMCACHED_SERVER", "memcache://127.0.0.1:11211"),
        jwt_key(),
    );
    let s = move || session_filter.clone();
    use warp::filters::query::query;
    use warp::path::{end, param};
    use warp::{body, get2 as get, path, post2 as post};
    let static_routes = path("static")
        .and(get())
        .and(path::tail())
        .and_then(static_file);
    #[rustfmt::skip]
    let routes = warp::any()
        .and(static_routes)
        .or(get().and(path("login")).and(end()).and(s()).and(query()).map(login))
        .or(post().and(path("login")).and(end()).and(s()).and(body::form()).map(do_login))
        .or(path("logout").and(end()).and(s()).map(logout))
        .or(get().and(end()).and(s()).map(all_years))
        .or(get().and(path("img")).and(param()).and(end()).and(s()).map(photo_details))
        .or(get().and(path("img")).and(param()).and(end()).and(s()).map(show_image))
        .or(get().and(path("0")).and(end()).and(s()).map(all_null_date))
        .or(get().and(param()).and(end()).and(s()).map(months_in_year))
        .or(get().and(param()).and(param()).and(end()).and(s()).map(days_in_month))
        .or(get().and(param()).and(param()).and(param()).and(end()).and(query()).and(s()).map(all_for_day))
        .or(get().and(path("person")).and(end()).and(s()).map(person_all))
        .or(get().and(path("person")).and(s()).and(param()).and(end()).and(query()).map(person_one))
        .or(get().and(path("place")).and(end()).and(s()).map(place_all))
        .or(get().and(path("place")).and(s()).and(param()).and(end()).and(query()).map(place_one))
        .or(get().and(path("tag")).and(end()).and(s()).map(tag_all))
        .or(get().and(path("tag")).and(s()).and(param()).and(end()).and(query()).map(tag_one))
        .or(get().and(path("random")).and(end()).and(s()).map(random_image))
        .or(get().and(path("thisday")).and(end()).and(s()).map(on_this_day))
        .or(get().and(path("next")).and(end()).and(s()).and(query()).map(next_image))
        .or(get().and(path("prev")).and(end()).and(s()).and(query()).map(prev_image))
        .or(get().and(path("ac")).and(path("tag")).and(s()).and(query()).map(auto_complete_tag))
        .or(get().and(path("ac")).and(path("person")).and(s()).and(query()).map(auto_complete_person))
        .or(path("adm").and(admin::routes(s())));
    let addr = env_or("RPHOTOS_LISTEN", "127.0.0.1:6767")
        .parse::<SocketAddr>()
        .map_err(|e| Error::Other(format!("{}", e)))?;
    warp::serve(routes.recover(customize_error)).run(addr);
    Ok(())
}

/// Create custom error pages.
fn customize_error(err: Rejection) -> Result<impl Reply, Rejection> {
    match err.status() {
        StatusCode::NOT_FOUND => {
            eprintln!("Got a 404: {:?}", err);
            Ok(Response::builder().status(StatusCode::NOT_FOUND).html(|o| {
                templates::error(
                    o,
                    StatusCode::NOT_FOUND,
                    "The resource you requested could not be located.",
                )
            }))
        }
        code => {
            eprintln!("Got a {}: {:?}", code.as_u16(), err);
            Ok(Response::builder()
                .status(code)
                .html(|o| templates::error(o, code, "Something went wrong.")))
        }
    }
}

fn not_found(context: &Context) -> Response<Vec<u8>> {
    Response::builder().status(StatusCode::NOT_FOUND).html(|o| {
        templates::not_found(
            o,
            context,
            StatusCode::NOT_FOUND,
            "The resource you requested could not be located.",
        )
    })
}

fn redirect_to_img(image: i32) -> Response<Vec<u8>> {
    redirect(&format!("/img/{}", image))
}

fn redirect(url: &str) -> Response<Vec<u8>> {
    Response::builder().redirect(url)
}

fn permission_denied() -> Response<Vec<u8>> {
    error_response(StatusCode::UNAUTHORIZED)
}

fn error_response(err: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(err)
        .html(|o| templates::error(o, err, "Sorry about this."))
}

fn login(context: Context, param: NextQ) -> Response<Vec<u8>> {
    info!("Got request for login form.  Param: {:?}", param);
    let next = sanitize_next(param.next.as_ref().map(AsRef::as_ref));
    Response::builder().html(|o| templates::login(o, &context, next, None))
}

#[derive(Debug, Default, Deserialize)]
struct NextQ {
    next: Option<String>,
}

fn do_login(context: Context, form: LoginForm) -> Response<Vec<u8>> {
    let next = sanitize_next(form.next.as_ref().map(AsRef::as_ref));
    use crate::schema::users::dsl::*;
    if let Ok(hash) = users
        .filter(username.eq(&form.user))
        .select(password)
        .first::<String>(context.db())
    {
        if djangohashers::check_password_tolerant(&form.password, &hash) {
            info!("User {} logged in", form.user);
            let token = context.make_token(&form.user).unwrap();
            return Response::builder()
                .header(
                    header::SET_COOKIE,
                    format!("EXAUTH={}; SameSite=Strict; HttpOpnly", token),
                )
                .redirect(next.unwrap_or("/"));
        }
        info!(
            "Login failed: Password verification failed for {:?}",
            form.user,
        );
    } else {
        info!("Login failed: No hash found for {:?}", form.user);
    }
    let message = Some("Login failed, please try again");
    Response::builder().html(|o| templates::login(o, &context, next, message))
}

/// The data submitted by the login form.
/// This does not derive Debug or Serialize, as the password is plain text.
#[derive(Deserialize)]
struct LoginForm {
    user: String,
    password: String,
    next: Option<String>,
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

fn logout(_context: Context) -> Response<Vec<u8>> {
    Response::builder()
        .header(
            header::SET_COOKIE,
            "EXAUTH=; Max-Age=0; SameSite=Strict; HttpOpnly",
        )
        .redirect("/")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeTag {
    Small,
    Medium,
    Large,
}
impl SizeTag {
    pub fn px(self) -> u32 {
        match self {
            SizeTag::Small => 240,
            SizeTag::Medium => 960,
            SizeTag::Large => 1900,
        }
    }
}

fn show_image(img: ImgName, context: Context) -> Response<Vec<u8>> {
    use crate::schema::photos::dsl::photos;
    if let Ok(tphoto) = photos.find(img.id).first::<Photo>(context.db()) {
        if context.is_authorized() || tphoto.is_public() {
            if img.size == SizeTag::Large {
                if context.is_authorized() {
                    use std::fs::File;
                    use std::io::Read;
                    // TODO: This should be done in a more async-friendly way.
                    let path = context.photos().get_raw_path(tphoto);
                    let mut buf = Vec::new();
                    if File::open(path)
                        .map(|mut f| f.read_to_end(&mut buf))
                        .is_ok()
                    {
                        return Response::builder()
                            .status(StatusCode::OK)
                            .header(
                                header::CONTENT_TYPE,
                                mime::IMAGE_JPEG.as_ref(),
                            )
                            .far_expires()
                            .body(buf)
                            .unwrap();
                    } else {
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                        );
                    }
                }
            } else {
                let data = get_image_data(context, &tphoto, img.size)
                    .expect("Get image data");
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, mime::IMAGE_JPEG.as_ref())
                    .far_expires()
                    .body(data)
                    .unwrap();
            }
        }
    }
    not_found(&context)
}

/// A client-side / url file name for a file.
/// Someting like 4711-s.jpg
#[derive(Debug, Eq, PartialEq)]
struct ImgName {
    id: i32,
    size: SizeTag,
}
use std::str::FromStr;
#[derive(Debug, Eq, PartialEq)]
struct BadImgName {}
impl FromStr for ImgName {
    type Err = BadImgName;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(pos) = s.find('-') {
            let (num, rest) = s.split_at(pos);
            let id = num.parse().map_err(|_| BadImgName {})?;
            let size = match rest {
                "-s.jpg" => SizeTag::Small,
                "-m.jpg" => SizeTag::Medium,
                "-l.jpg" => SizeTag::Large,
                _ => return Err(BadImgName {}),
            };
            return Ok(ImgName { id, size });
        }
        Err(BadImgName {})
    }
}

#[test]
fn parse_good_imgname() {
    assert_eq!(
        "4711-s.jpg".parse(),
        Ok(ImgName {
            id: 4711,
            size: SizeTag::Small,
        })
    )
}

#[test]
fn parse_bad_imgname_1() {
    assert_eq!("4711-q.jpg".parse::<ImgName>(), Err(BadImgName {}))
}
#[test]
fn parse_bad_imgname_2() {
    assert_eq!("blurgel".parse::<ImgName>(), Err(BadImgName {}))
}

fn get_image_data(
    context: Context,
    photo: &Photo,
    size: SizeTag,
) -> Result<Vec<u8>, image::ImageError> {
    context.cached_or(&photo.cache_key(size), || {
        let size = size.px();
        context.photos().scale_image(photo, size, size)
    })
}

fn tag_all(context: Context) -> Response<Vec<u8>> {
    use crate::schema::tags::dsl::{id, tag_name, tags};
    let query = tags.into_boxed();
    let query = if context.is_authorized() {
        query
    } else {
        use crate::schema::photo_tags::dsl as tp;
        use crate::schema::photos::dsl as p;
        query.filter(id.eq_any(tp::photo_tags.select(tp::tag_id).filter(
            tp::photo_id.eq_any(p::photos.select(p::id).filter(p::is_public)),
        )))
    };
    Response::builder().html(|o| {
        templates::tags(
            o,
            &context,
            &query.order(tag_name).load(context.db()).expect("List tags"),
        )
    })
}

fn tag_one(
    context: Context,
    tslug: String,
    range: ImgRange,
) -> Response<Vec<u8>> {
    use crate::schema::tags::dsl::{slug, tags};
    if let Ok(tag) = tags.filter(slug.eq(tslug)).first::<Tag>(context.db()) {
        use crate::schema::photo_tags::dsl::{photo_id, photo_tags, tag_id};
        use crate::schema::photos::dsl::id;
        let photos = Photo::query(context.is_authorized()).filter(
            id.eq_any(photo_tags.select(photo_id).filter(tag_id.eq(tag.id))),
        );
        let (links, coords) = links_by_time(&context, photos, range, true);
        Response::builder()
            .html(|o| templates::tag(o, &context, &links, &coords, &tag))
    } else {
        not_found(&context)
    }
}

fn place_all(context: Context) -> Response<Vec<u8>> {
    use crate::schema::places::dsl::{id, place_name, places};
    let query = places.into_boxed();
    let query = if context.is_authorized() {
        query
    } else {
        use crate::schema::photo_places::dsl as pp;
        use crate::schema::photos::dsl as p;
        query.filter(id.eq_any(pp::photo_places.select(pp::place_id).filter(
            pp::photo_id.eq_any(p::photos.select(p::id).filter(p::is_public)),
        )))
    };
    Response::builder().html(|o| {
        templates::places(
            o,
            &context,
            &query
                .order(place_name)
                .load(context.db())
                .expect("List places"),
        )
    })
}

/// Handler for static files.
/// Create a response from the file data with a correct content type
/// and a far expires header (or a 404 if the file does not exist).
fn static_file(name: Tail) -> Result<impl Reply, Rejection> {
    use templates::statics::StaticFile;
    if let Some(data) = StaticFile::get(name.as_str()) {
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, data.mime.as_ref())
            .far_expires()
            .body(data.content))
    } else {
        println!("Static file {:?} not found", name);
        Err(warp::reject::not_found())
    }
}

fn place_one(
    context: Context,
    tslug: String,
    range: ImgRange,
) -> Response<Vec<u8>> {
    use crate::schema::places::dsl::{places, slug};
    if let Ok(place) =
        places.filter(slug.eq(tslug)).first::<Place>(context.db())
    {
        use crate::schema::photo_places::dsl::{
            photo_id, photo_places, place_id,
        };
        use crate::schema::photos::dsl::id;
        let photos = Photo::query(context.is_authorized()).filter(id.eq_any(
            photo_places.select(photo_id).filter(place_id.eq(place.id)),
        ));
        let (links, coord) = links_by_time(&context, photos, range, true);
        Response::builder()
            .html(|o| templates::place(o, &context, &links, &coord, &place))
    } else {
        not_found(&context)
    }
}

fn person_all(context: Context) -> Response<Vec<u8>> {
    use crate::schema::people::dsl::{id, people, person_name};
    let query = people.into_boxed();
    let query = if context.is_authorized() {
        query
    } else {
        use crate::schema::photo_people::dsl as pp;
        use crate::schema::photos::dsl as p;
        query.filter(id.eq_any(pp::photo_people.select(pp::person_id).filter(
            pp::photo_id.eq_any(p::photos.select(p::id).filter(p::is_public)),
        )))
    };
    Response::builder().html(|o| {
        templates::people(
            o,
            &context,
            &query
                .order(person_name)
                .load(context.db())
                .expect("list people"),
        )
    })
}

fn person_one(
    context: Context,
    tslug: String,
    range: ImgRange,
) -> Response<Vec<u8>> {
    use crate::schema::people::dsl::{people, slug};
    let c = context.db();
    if let Ok(person) = people.filter(slug.eq(tslug)).first::<Person>(c) {
        use crate::schema::photo_people::dsl::{
            person_id, photo_id, photo_people,
        };
        use crate::schema::photos::dsl::id;
        let photos = Photo::query(context.is_authorized()).filter(
            id.eq_any(
                photo_people
                    .select(photo_id)
                    .filter(person_id.eq(person.id)),
            ),
        );
        let (links, coords) = links_by_time(&context, photos, range, true);
        Response::builder()
            .html(|o| templates::person(o, &context, &links, &coords, &person))
    } else {
        not_found(&context)
    }
}

fn random_image(context: Context) -> Response<Vec<u8>> {
    use crate::schema::photos::dsl::id;
    use diesel::expression::dsl::sql;
    use diesel::sql_types::Integer;
    if let Ok(photo) = Photo::query(context.is_authorized())
        .select(id)
        .limit(1)
        .order(sql::<Integer>("random()"))
        .first(context.db())
    {
        info!("Random: {:?}", photo);
        redirect_to_img(photo)
    } else {
        not_found(&context)
    }
}

fn photo_details(id: i32, context: Context) -> Response<Vec<u8>> {
    use crate::schema::photos::dsl::photos;
    let c = context.db();
    if let Ok(tphoto) = photos.find(id).first::<Photo>(c) {
        if context.is_authorized() || tphoto.is_public() {
            return Response::builder().html(|o| {
                templates::details(
                    o,
                    &context,
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
    not_found(&context)
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

fn auto_complete_tag(context: Context, query: AcQ) -> impl Reply {
    use crate::schema::tags::dsl::{tag_name, tags};
    let q = tags
        .select(tag_name)
        .filter(tag_name.ilike(query.q + "%"))
        .order(tag_name)
        .limit(10);
    reply::json(&q.load::<String>(context.db()).unwrap())
}

fn auto_complete_person(context: Context, query: AcQ) -> impl Reply {
    use crate::schema::people::dsl::{people, person_name};
    let q = people
        .select(person_name)
        .filter(person_name.ilike(query.q + "%"))
        .order(person_name)
        .limit(10);
    reply::json(&q.load::<String>(context.db()).unwrap())
}

#[derive(Deserialize)]
struct AcQ {
    q: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct ImgRange {
    pub from: Option<i32>,
    pub to: Option<i32>,
}
