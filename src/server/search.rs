use super::render_ructe::RenderRucte;
use super::views_by_category::AcQ;
use super::{links_by_time, Context, ImgRange};
use crate::adm::result::Error;
use crate::models::{Person, Photo, Place, Tag};
use crate::schema::people::dsl as h; // h as in human
use crate::schema::photo_people::dsl as pp;
use crate::schema::photo_places::dsl as pl;
use crate::schema::photo_tags::dsl as pt;
use crate::schema::photos::dsl as p;
use crate::schema::places::dsl as l;
use crate::schema::tags::dsl as t;
use crate::templates;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use serde::Serialize;
use warp::http::Response;
use warp::{reply, Reply};

#[derive(Debug, Serialize)]
struct SearchTag {
    /// Kind (may be "p" for person, "t" for tag, "l" for location).
    k: char,
    /// Title of the the tag
    t: String,
    /// Slug
    s: String,
}

pub fn auto_complete_any(context: Context, query: AcQ) -> impl Reply {
    let qs = format!("%{}%", query.q);

    let query = t::tags
        .select((t::tag_name, t::slug))
        .filter(t::tag_name.ilike(&qs))
        .into_boxed();
    let query = if context.is_authorized() {
        query
    } else {
        use crate::schema::photo_tags::dsl as tp;
        query.filter(t::id.eq_any(tp::photo_tags.select(tp::tag_id).filter(
            tp::photo_id.eq_any(p::photos.select(p::id).filter(p::is_public)),
        )))
    };
    let db = context.db().unwrap();
    let mut tags = query
        .order(t::tag_name)
        .limit(10)
        .load::<(String, String)>(&db)
        .unwrap()
        .into_iter()
        .map(|(t, s)| SearchTag { k: 't', t, s })
        .collect::<Vec<_>>();
    tags.extend({
        let query = h::people
            .select((h::person_name, h::slug))
            .filter(h::person_name.ilike(&qs))
            .into_boxed();
        let query =
            if context.is_authorized() {
                query
            } else {
                query.filter(h::id.eq_any(
                    pp::photo_people.select(pp::person_id).filter(
                        pp::photo_id.eq_any(
                            p::photos.select(p::id).filter(p::is_public),
                        ),
                    ),
                ))
            };
        query
            .order(h::person_name)
            .limit(10)
            .load::<(String, String)>(&db)
            .unwrap()
            .into_iter()
            .map(|(t, s)| SearchTag { k: 'p', t, s })
    });
    tags.extend({
        let query = l::places
            .select((l::place_name, l::slug))
            .filter(l::place_name.ilike(&qs))
            .into_boxed();
        let query =
            if context.is_authorized() {
                query
            } else {
                use crate::schema::photo_places::dsl as lp;
                query.filter(l::id.eq_any(
                    lp::photo_places.select(lp::place_id).filter(
                        lp::photo_id.eq_any(
                            p::photos.select(p::id).filter(p::is_public),
                        ),
                    ),
                ))
            };
        query
            .order(l::place_name)
            .limit(10)
            .load::<(String, String)>(&db)
            .unwrap()
            .into_iter()
            .map(|(t, s)| SearchTag { k: 'l', t, s })
    });
    reply::json(&tags)
}

pub fn search(context: Context, query: Vec<(String, String)>) -> impl Reply {
    let query = SearchQuery::load(query, &context.db().unwrap()).unwrap();
    let range = ImgRange::default();

    let mut photos = Photo::query(context.is_authorized());
    if let Some(since) = query.since {
        photos = photos.filter(p::date.ge(since));
    }
    if let Some(until) = query.until {
        photos = photos.filter(p::date.le(until));
    }
    for tag in &query.t {
        photos = photos.filter(
            p::id.eq_any(
                pt::photo_tags
                    .select(pt::photo_id)
                    .filter(pt::tag_id.eq(tag.id)),
            ),
        );
    }
    for location in &query.l {
        photos = photos.filter(
            p::id.eq_any(
                pl::photo_places
                    .select(pl::photo_id)
                    .filter(pl::place_id.eq(location.id)),
            ),
        );
    }
    for person in &query.p {
        photos = photos.filter(
            p::id.eq_any(
                pp::photo_people
                    .select(pp::photo_id)
                    .filter(pp::person_id.eq(person.id)),
            ),
        );
    }
    if let Some(pos) = query.pos {
        use crate::schema::positions::dsl as pos;
        let pos_ids = pos::positions.select(pos::photo_id);
        if pos {
            photos = photos.filter(p::id.eq_any(pos_ids));
        } else {
            photos = photos.filter(p::id.ne_all(pos_ids));
        }
    }

    let (mut links, coords) = links_by_time(&context, photos, range, true);
    let addendum = &query
        .t
        .iter()
        .map(|v| format!("&t={}", v.slug))
        .chain(query.l.iter().map(|v| format!("&l={}", v.slug)))
        .chain(query.p.iter().map(|v| format!("&p={}", v.slug)))
        .chain(query.pos.map(|v| format!("&pos={}", v)))
        .collect::<String>();
    for link in &mut links {
        if link.href.starts_with("/search/?") {
            link.href += &addendum;
        }
    }
    Response::builder()
        .html(|o| templates::search(o, &context, &query, &links, &coords))
}

#[derive(Debug, Default)]
pub struct SearchQuery {
    /// Keys
    pub t: Vec<Tag>,
    /// People
    pub p: Vec<Person>,
    /// Places (locations)
    pub l: Vec<Place>,
    pub since: Option<NaiveDateTime>,
    pub until: Option<NaiveDateTime>,
    pub pos: Option<bool>,
    /// Query (free-text, don't know what to do)
    pub q: String,
}

impl SearchQuery {
    fn load(
        query: Vec<(String, String)>,
        db: &PgConnection,
    ) -> Result<Self, Error> {
        let mut result = SearchQuery::default();
        let (mut s_d, mut s_t, mut u_d, mut u_t) = (None, None, None, None);
        for (key, val) in &query {
            match key.as_ref() {
                "since_date" => s_d = Some(val.as_ref()),
                "since_time" => s_t = Some(val.as_ref()),
                "until_date" => u_d = Some(val.as_ref()),
                "until_time" => u_t = Some(val.as_ref()),
                _ => (),
            }
        }
        let since_midnight = NaiveTime::from_hms_milli(0, 0, 0, 0);
        result.since = datetime_from_parts(s_d, s_t, since_midnight);
        let until_midnight = NaiveTime::from_hms_milli(23, 59, 59, 999);
        result.until = datetime_from_parts(u_d, u_t, until_midnight);
        for (key, val) in query {
            match key.as_ref() {
                "q" => {
                    if val.contains("!pos") {
                        result.pos = Some(false);
                    } else if val.contains("pos") {
                        result.pos = Some(true);
                    }
                    result.q = val;
                }
                "t" => {
                    result.t.push(t::tags.filter(t::slug.eq(val)).first(db)?)
                }
                "p" => {
                    result.p.push(h::people.filter(h::slug.eq(val)).first(db)?)
                }
                "l" => {
                    result.l.push(l::places.filter(l::slug.eq(val)).first(db)?)
                }
                "pos" => {
                    result.pos = Some(
                        val.parse()
                            .map_err(|e| Error::Other(format!("{}", e)))?,
                    );
                }
                "from" => {
                    result.since = p::photos
                        .select(p::date)
                        .filter(p::id.eq(val.parse::<i32>()?))
                        .first(db)?
                }
                "to" => {
                    result.until = p::photos
                        .select(p::date)
                        .filter(p::id.eq(val.parse::<i32>()?))
                        .first(db)?
                }
                _ => (), // ignore unknown query parameters
            }
        }
        Ok(result)
    }
}

fn datetime_from_parts(
    date: Option<&str>,
    time: Option<&str>,
    defaulttime: NaiveTime,
) -> Option<NaiveDateTime> {
    date.and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        .map(|date| {
            date.and_time(
                time.and_then(|s| {
                    NaiveTime::parse_from_str(s, "%H:%M:%S").ok()
                })
                .unwrap_or(defaulttime),
            )
        })
}
