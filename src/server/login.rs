use super::{wrap, BuilderExt, Context, ContextFilter, RenderRucte, Result};
use crate::templates;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use lazy_regex::regex_is_match;
use serde::Deserialize;
use tracing::info;
use warp::filters::BoxedFilter;
use warp::http::header;
use warp::http::response::Builder;
use warp::path::end;
use warp::query::query;
use warp::reply::Response;
use warp::{body, get, path, post, Filter};

pub fn routes(s: ContextFilter) -> BoxedFilter<(Response,)> {
    let s = move || s.clone();
    let get_form = get().and(s()).and(query()).map(get_login);
    let post_form = post().and(s()).and(body::form()).map(post_login);
    let login = path("login")
        .and(end())
        .and(get_form.or(post_form).unify().map(wrap));
    let logout = path("logout").and(end()).and(s()).map(logout);
    login.or(logout).unify().boxed()
}

fn get_login(context: Context, param: NextQ) -> Result<Response> {
    info!("Got request for login form.  Param: {:?}", param);
    let next = sanitize_next(param.next.as_ref().map(AsRef::as_ref));
    Ok(Builder::new()
        .html(|o| templates::login_html(o, &context, next, None))?)
}

#[derive(Debug, Default, Deserialize)]
struct NextQ {
    next: Option<String>,
}

fn post_login(context: Context, form: LoginForm) -> Result<Response> {
    let next = sanitize_next(form.next.as_ref().map(AsRef::as_ref));
    let mut db = context.db()?;
    if let Some(user) = form.validate(&mut db) {
        let token = context.make_token(&user)?;
        return Ok(Builder::new()
            .header(
                header::SET_COOKIE,
                format!("EXAUTH={token}; SameSite=Strict; HttpOnly"),
            )
            .redirect(next.unwrap_or("/")));
    }
    let message = Some("Login failed, please try again");
    Ok(Builder::new()
        .html(|o| templates::login_html(o, &context, next, message))?)
}

/// The data submitted by the login form.
/// This does not derive Debug or Serialize, as the password is plain text.
#[derive(Deserialize)]
pub struct LoginForm {
    user: String,
    password: String,
    next: Option<String>,
}

impl LoginForm {
    /// Retur user if and only if password is correct for user.
    pub fn validate(&self, db: &mut PgConnection) -> Option<String> {
        use crate::schema::users::dsl::*;
        if let Ok(hash) = users
            .filter(username.eq(&self.user))
            .select(password)
            .first::<String>(db)
        {
            if djangohashers::check_password_tolerant(&self.password, &hash) {
                info!("User {} logged in", self.user);
                return Some(self.user.clone());
            }
            info!(
                "Login failed: Password verification failed for {:?}",
                self.user,
            );
        } else {
            info!("Login failed: No hash found for {:?}", self.user);
        }
        None
    }
}

fn sanitize_next(next: Option<&str>) -> Option<&str> {
    if let Some(next) = next {
        if regex_is_match!(r"^/([a-z0-9._-]+/?)*$", next) {
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

fn logout(_context: Context) -> Response {
    Builder::new()
        .header(
            header::SET_COOKIE,
            "EXAUTH=; Max-Age=0; SameSite=Strict; HttpOnly",
        )
        .redirect("/")
}
