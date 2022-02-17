#[macro_use]
mod error;
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
use self::error::{for_rejection, ViewError, ViewResult};
pub use self::photolink::PhotoLink;
use self::render_ructe::BuilderExt;
use self::search::*;
use self::views_by_category::*;
use self::views_by_date::monthname;
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
use std::path::PathBuf;
use structopt::StructOpt;
use warp::filters::path::Tail;
use warp::http::{header, response::Builder, StatusCode};
use warp::reply::Response;
use warp::{self, Filter, Reply};

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
    pidfile: Option<PathBuf>,
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
        handle_pid_file(pidfile, args.replace)?;
    }
    let session_filter = create_session_filter(args);
    let s = move || session_filter.clone();
    use warp::filters::query::query;
    use warp::path::{end, param};
    use warp::{get, path};
    let static_routes = path("static")
        .and(path::tail())
        .and(get())
        .then(static_file)
        .map(wrap);
    #[rustfmt::skip]
    let routes = warp::any()
        .and(static_routes)
        .or(login::routes(s()))
        .or(path("img").and(
            param().and(end()).and(get()).and(s()).map(photo_details)
                .or(param().and(end()).and(get()).and(s()).then(image::show_image))
                .unify()
                .map(wrap)))
        .or(views_by_date::routes(s()))
        .or(path("person").and(person_routes(s())))
        .or(path("place").and(place_routes(s())))
        .or(path("tag").and(tag_routes(s())))
        .or(path("random").and(end()).and(get()).and(s()).map(random_image).map(wrap))
        .or(path("ac").and(autocomplete::routes(s())))
        .or(path("search").and(end()).and(get()).and(s()).and(query()).map(search).map(wrap))
        .or(path("api").and(api::routes(s())))
        .or(path("adm").and(admin::routes(s())));
    warp::serve(routes.recover(for_rejection))
        .run(args.listen)
        .await;
    Ok(())
}

type Result<T, E = ViewError> = std::result::Result<T, E>;

fn wrap(result: Result<impl Reply>) -> Response {
    match result {
        Ok(reply) => reply.into_response(),
        Err(err) => err.into_response(),
    }
}

fn redirect_to_img(image: i32) -> Response {
    redirect(&format!("/img/{}", image))
}

fn redirect(url: &str) -> Response {
    Builder::new().redirect(url)
}

/// Handler for static files.
/// Create a response from the file data with a correct content type
/// and a far expires header (or a 404 if the file does not exist).
async fn static_file(name: Tail) -> Result<Response> {
    use templates::statics::StaticFile;
    let data = or_404!(StaticFile::get(name.as_str()));
    Builder::new()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, data.mime.as_ref())
        .far_expires()
        .body(data.content.into())
        .ise()
}

fn random_image(context: Context) -> Result<Response> {
    use crate::schema::photos::dsl::id;
    use diesel::expression::dsl::sql;
    use diesel::sql_types::Integer;
    let photo = Photo::query(context.is_authorized())
        .select(id)
        .limit(1)
        .order(sql::<Integer>("random()"))
        .first(&context.db()?)?;

    info!("Random: {:?}", photo);
    Ok(redirect_to_img(photo))
}

fn photo_details(id: i32, context: Context) -> Result<Response> {
    use crate::schema::photos::dsl::photos;
    let c = context.db()?;
    let tphoto = or_404q!(photos.find(id).first::<Photo>(&c), context);

    if context.is_authorized() || tphoto.is_public() {
        Ok(Builder::new().html(|o| {
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
        })?)
    } else {
        Err(ViewError::NotFound(Some(context)))
    }
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
