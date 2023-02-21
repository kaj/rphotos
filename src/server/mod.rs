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
use crate::models::{Photo, PhotoDetails};
use crate::pidfiles::handle_pid_file;
use crate::templates::{self, Html, RenderRucte};
use chrono::Datelike;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::info;
use warp::filters::path::Tail;
use warp::http::{header, response::Builder, StatusCode};
use warp::reply::Response;
use warp::{self, Filter, Reply};

#[derive(clap::Parser)]
pub struct Args {
    #[clap(flatten)]
    db: DbOpt,
    #[clap(flatten)]
    cache: CacheOpt,
    #[clap(flatten)]
    photos: DirOpt,
    #[clap(flatten)]
    overpass: OverpassOpt,

    /// Write (and read, if --replace) a pid file with the name
    /// given as <PIDFILE>.
    #[clap(long)]
    pidfile: Option<PathBuf>,
    /// Kill old server (identified by pid file) before running.
    #[clap(long, short)]
    replace: bool,
    /// Socket addess for rphotos to listen on.
    #[clap(long, env = "RPHOTOS_LISTEN", default_value = "127.0.0.1:6767")]
    listen: SocketAddr,
    /// Signing key for jwt
    #[clap(long, env = "JWT_KEY", hide_env_values = true)]
    jwt_key: String,
}

pub async fn run(args: &Args) -> Result<(), Error> {
    if let Some(pidfile) = &args.pidfile {
        handle_pid_file(pidfile, args.replace)?;
    }
    let session_filter = create_session_filter(args)?;
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
            param().and(end()).and(get()).and(s()).then(photo_details)
                .or(param().and(end()).and(get()).and(s()).then(image::show_image))
                .unify()
                .map(wrap)))
        .or(views_by_date::routes(s()))
        .or(path("person").and(person_routes(s())))
        .or(path("place").and(place_routes(s())))
        .or(path("tag").and(tag_routes(s())))
        .or(path("random").and(end()).and(get()).and(s()).then(random_image).map(wrap))
        .or(path("ac").and(autocomplete::routes(s())))
        .or(path("search").and(end()).and(get()).and(s()).and(query()).then(search).map(wrap))
        .or(path("api").and(api::routes(s())))
        .or(path("adm").and(admin::routes(s())))
        .or(path("robots.txt")
            .and(end())
            .and(get())
            .map(robots_txt)
            .map(wrap));
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
    redirect(&format!("/img/{image}"))
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

async fn random_image(context: Context) -> Result<Response> {
    use crate::schema::photos::dsl::id;
    sql_function! { fn random() -> Integer };

    let photo = Photo::query(context.is_authorized())
        .select(id)
        .limit(1)
        .order(random())
        .first(&mut context.db().await?)
        .await?;

    info!("Random: {:?}", photo);
    Ok(redirect_to_img(photo))
}

async fn photo_details(id: i32, context: Context) -> Result<Response> {
    let mut c = context.db().await?;
    let photo = or_404q!(PhotoDetails::load(id, &mut c).await, context);

    if context.is_authorized() || photo.is_public() {
        Ok(Builder::new().html(|o| {
            templates::details_html(
                o,
                &context,
                &photo
                    .date
                    .map(|d| {
                        vec![
                            Link::year(d.year()),
                            Link::month(d.year(), d.month()),
                            Link::day(d.year(), d.month(), d.day()),
                            Link::prev(photo.id),
                            Link::next(photo.id),
                        ]
                    })
                    .unwrap_or_default(),
                &photo,
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
            "<a href='/{year}/' title='Images from {year}' accessKey='y'>{year}</a>",
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
            "<a href='/prev?from={from}' title='Previous image (by time)'>\
             \u{2190}</a>",
        ))
    }
    fn next(from: i32) -> Self {
        Html(format!(
            "<a href='/next?from={from}' title='Next image (by time)' \
             accessKey='n'>\u{2192}</a>",
        ))
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct ImgRange {
    pub from: Option<i32>,
    pub to: Option<i32>,
}

fn robots_txt() -> Result<Response> {
    Builder::new()
        .header(header::CONTENT_TYPE, mime::TEXT_PLAIN.as_ref())
        .body(
            "User-agent: *\n\
             Disallow: /login\n\
             Disallow: /logout\n\
             Disallow: /ac\n"
                .into(),
        )
        .ise()
}
