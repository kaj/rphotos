use super::render_ructe::RenderRucte;
use super::Context;
use crate::templates;
use diesel::prelude::*;
use log::info;
use serde::Deserialize;
use warp::http::{header, Response};

pub fn get_login(context: Context, param: NextQ) -> Response<Vec<u8>> {
    info!("Got request for login form.  Param: {:?}", param);
    let next = sanitize_next(param.next.as_ref().map(AsRef::as_ref));
    Response::builder().html(|o| templates::login(o, &context, next, None))
}

#[derive(Debug, Default, Deserialize)]
pub struct NextQ {
    next: Option<String>,
}

pub fn post_login(context: Context, form: LoginForm) -> Response<Vec<u8>> {
    let next = sanitize_next(form.next.as_ref().map(AsRef::as_ref));
    use crate::schema::users::dsl::*;
    if let Ok(hash) = users
        .filter(username.eq(&form.user))
        .select(password)
        .first::<String>(&context.db().unwrap())
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
pub struct LoginForm {
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

pub fn logout(_context: Context) -> Response<Vec<u8>> {
    Response::builder()
        .header(
            header::SET_COOKIE,
            "EXAUTH=; Max-Age=0; SameSite=Strict; HttpOpnly",
        )
        .redirect("/")
}
