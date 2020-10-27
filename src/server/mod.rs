mod admin;
mod api;
mod autocomplete;
mod context;
mod image;
mod login;
mod photolink;
mod render_ructe;
pub mod search;
mod splitlist;
mod urlstring;
mod views_by_category;
mod views_by_date;

use self::context::create_session_filter;
pub use self::context::{Context, ContextFilter};
pub use self::photolink::PhotoLink;
use self::render_ructe::BuilderExt;
use self::search::*;
use self::views_by_category::*;
use self::views_by_date::*;
use super::{CacheOpt, DbOpt, DirOpt};
use crate::adm::result::Error;
use crate::fetch_places::OverpassOpt;
use crate::models::Photo;
use crate::pidfiles::handle_pid_file;
use crate::templates::{self, Html, RenderRucte};
use chrono::Datelike;
use diesel::prelude::*;
use log::info;
use serde::Deserialize;
use std::net::SocketAddr;
use structopt::StructOpt;
use warp::filters::path::Tail;
use warp::http::{header, response::Builder, StatusCode};
use warp::reply::Response;
use warp::{self, Filter, Rejection, Reply};

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct Args {
    #[structopt(flatten)]
    db: DbOpt,
    #[structopt(flatten)]
    cache: CacheOpt,
    #[structopt(flatten)]
    photos: DirOpt,
    #[structopt(flatten)]
    overpass: OverpassOpt,

    /// Write (and read, if --replace) a pid file with the name
    /// given as <PIDFILE>.
    #[structopt(long)]
    pidfile: Option<String>,
    /// Kill old server (identified by pid file) before running.
    #[structopt(long, short)]
    replace: bool,
    /// Socket addess for rphotos to listen on.
    #[structopt(
        long,
        env = "RPHOTOS_LISTEN",
        default_value = "127.0.0.1:6767"
    )]
    listen: SocketAddr,
    /// Signing key for jwt
    #[structopt(long, env = "JWT_KEY", hide_env_values = true)]
    jwt_key: String,
}

pub async fn run(args: &Args) -> Result<(), Error> {
    if let Some(pidfile) = &args.pidfile {
        handle_pid_file(&pidfile, args.replace).unwrap()
    }
    let session_filter = create_session_filter(args);
    let s = move || session_filter.clone();
    use warp::filters::query::query;
    use warp::path::{end, param};
    use warp::{body, get, path, post};
    let static_routes = path("static")
        .and(get())
        .and(path::tail())
        .and_then(static_file);
    #[rustfmt::skip]
    let routes = warp::any()
        .and(static_routes)
        .or(get().and(path("login")).and(end()).and(s()).and(query()).map(login::get_login))
        .or(post().and(path("login")).and(end()).and(s()).and(body::form()).map(login::post_login))
        .or(path("logout").and(end()).and(s()).map(login::logout))
        .or(get().and(end()).and(s()).map(all_years))
        .or(get().and(path("img")).and(param()).and(end()).and(s()).map(photo_details))
        .or(get().and(path("img")).and(param()).and(end()).and(s()).and_then(image::show_image))
        .or(get().and(path("0")).and(end()).and(s()).map(all_null_date))
        .or(get().and(param()).and(end()).and(s()).map(months_in_year))
        .or(get().and(param()).and(param()).and(end()).and(s()).map(days_in_month))
        .or(get().and(param()).and(param()).and(param()).and(end()).and(query()).and(s()).map(all_for_day))
        .or(path("person").and(person_routes(s())))
        .or(path("place").and(place_routes(s())))
        .or(path("tag").and(tag_routes(s())))
        .or(get().and(path("random")).and(end()).and(s()).map(random_image))
        .or(get().and(path("thisday")).and(end()).and(s()).map(on_this_day))
        .or(get().and(path("next")).and(end()).and(s()).and(query()).map(next_image))
        .or(get().and(path("prev")).and(end()).and(s()).and(query()).map(prev_image))
        .or(path("ac").and(autocomplete::routes(s())))
        .or(path("search").and(end()).and(get()).and(s()).and(query()).map(search))
        .or(path("api").and(api::routes(s())))
        .or(path("adm").and(admin::routes(s())));
    warp::serve(routes.recover(customize_error))
        .run(args.listen)
        .await;
    Ok(())
}

/// Create custom error pages.
async fn customize_error(err: Rejection) -> Result<Response, Rejection> {
    if err.is_not_found() {
        eprintln!("Got a 404: {:?}", err);
        Builder::new().status(StatusCode::NOT_FOUND).html(|o| {
            templates::error(
                o,
                StatusCode::NOT_FOUND,
                "The resource you requested could not be located.",
            )
        })
    } else {
        let code = StatusCode::INTERNAL_SERVER_ERROR; // FIXME
        eprintln!("Got a {}: {:?}", code.as_u16(), err);
        Builder::new()
            .status(code)
            .html(|o| templates::error(o, code, "Something went wrong."))
    }
}

fn not_found(context: &Context) -> Response {
    Builder::new()
        .status(StatusCode::NOT_FOUND)
        .html(|o| {
            templates::not_found(
                o,
                context,
                StatusCode::NOT_FOUND,
                "The resource you requested could not be located.",
            )
        })
        .unwrap()
}

fn redirect_to_img(image: i32) -> Response {
    redirect(&format!("/img/{}", image))
}

fn redirect(url: &str) -> Response {
    Builder::new().redirect(url)
}

fn permission_denied() -> Result<Response, Rejection> {
    error_response(StatusCode::UNAUTHORIZED)
}

fn error_response(err: StatusCode) -> Result<Response, Rejection> {
    Builder::new()
        .status(err)
        .html(|o| templates::error(o, err, "Sorry about this."))
}

/// Handler for static files.
/// Create a response from the file data with a correct content type
/// and a far expires header (or a 404 if the file does not exist).
async fn static_file(name: Tail) -> Result<impl Reply, Rejection> {
    use templates::statics::StaticFile;
    if let Some(data) = StaticFile::get(name.as_str()) {
        Ok(Builder::new()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, data.mime.as_ref())
            .far_expires()
            .body(data.content))
    } else {
        println!("Static file {:?} not found", name);
        Err(warp::reject::not_found())
    }
}

fn random_image(context: Context) -> Response {
    use crate::schema::photos::dsl::id;
    use diesel::expression::dsl::sql;
    use diesel::sql_types::Integer;
    if let Ok(photo) = Photo::query(context.is_authorized())
        .select(id)
        .limit(1)
        .order(sql::<Integer>("random()"))
        .first(&context.db().unwrap())
    {
        info!("Random: {:?}", photo);
        redirect_to_img(photo)
    } else {
        not_found(&context)
    }
}

fn photo_details(id: i32, context: Context) -> Response {
    use crate::schema::photos::dsl::photos;
    let c = context.db().unwrap();
    if let Ok(tphoto) = photos.find(id).first::<Photo>(&c) {
        if context.is_authorized() || tphoto.is_public() {
            return Builder::new()
                .html(|o| {
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
                        &tphoto.load_people(&c).unwrap(),
                        &tphoto.load_places(&c).unwrap(),
                        &tphoto.load_tags(&c).unwrap(),
                        &tphoto.load_position(&c),
                        &tphoto.load_attribution(&c),
                        &tphoto.load_camera(&c),
                        &tphoto,
                    )
                })
                .unwrap();
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

#[derive(Debug, Default, Deserialize)]
pub struct ImgRange {
    pub from: Option<i32>,
    pub to: Option<i32>,
}
