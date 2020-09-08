use super::RenderRucte;
use super::{links_by_time, Context, ImgRange};
use crate::adm::result::Error;
use crate::models::{Facet, Person, Photo, Place, Tag};
use crate::schema::photo_people::dsl as pp;
use crate::schema::photo_places::dsl as pl;
use crate::schema::photo_tags::dsl as pt;
use crate::schema::photos::dsl as p;
use crate::templates;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use log::warn;
use warp::http::response::Builder;
use warp::reply::Response;

pub fn search(context: Context, query: Vec<(String, String)>) -> Response {
    let query = SearchQuery::load(query, &context.db().unwrap()).unwrap();
    let range = ImgRange::default();

    let mut photos = Photo::query(context.is_authorized());
    if let Some(since) = query.since.as_ref() {
        photos = photos.filter(p::date.ge(since));
    }
    if let Some(until) = query.until.as_ref() {
        photos = photos.filter(p::date.le(until));
    }
    for tag in &query.t {
        let ids = pt::photo_tags
            .select(pt::photo_id)
            .filter(pt::tag_id.eq(tag.item.id));
        photos = if tag.inc {
            photos.filter(p::id.eq_any(ids))
        } else {
            photos.filter(p::id.ne_all(ids))
        };
    }
    for location in &query.l {
        let ids = pl::photo_places
            .select(pl::photo_id)
            .filter(pl::place_id.eq(location.item.id));
        photos = if location.inc {
            photos.filter(p::id.eq_any(ids))
        } else {
            photos.filter(p::id.ne_all(ids))
        };
    }
    for person in &query.p {
        let ids = pp::photo_people
            .select(pp::photo_id)
            .filter(pp::person_id.eq(person.item.id));
        photos = if person.inc {
            photos.filter(p::id.eq_any(ids))
        } else {
            photos.filter(p::id.ne_all(ids))
        }
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
    let addendum = query.to_query_string();
    for link in &mut links {
        if link.href.starts_with("/search/?") {
            link.href += &addendum;
        }
    }
    Builder::new()
        .html(|o| templates::search(o, &context, &query, &links, &coords))
        .unwrap()
}

#[derive(Debug, Default)]
pub struct SearchQuery {
    /// Keys
    pub t: Vec<Filter<Tag>>,
    /// People
    pub p: Vec<Filter<Person>>,
    /// Places (locations)
    pub l: Vec<Filter<Place>>,
    pub since: QueryDateTime,
    pub until: QueryDateTime,
    pub pos: Option<bool>,
    /// Query (free-text, don't know what to do)
    pub q: String,
}

#[derive(Debug)]
pub struct Filter<T> {
    pub inc: bool,
    pub item: T,
}

impl<T: Facet> Filter<T> {
    fn load(val: &str, db: &PgConnection) -> Option<Filter<T>> {
        let (inc, slug) = if val.starts_with('!') {
            (false, &val[1..])
        } else {
            (true, val)
        };
        match T::by_slug(slug, db) {
            Ok(item) => Some(Filter { inc, item }),
            Err(err) => {
                warn!("No filter {:?}: {:?}", slug, err);
                None
            }
        }
    }
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
        result.since = QueryDateTime::since_from_parts(s_d, s_t);
        result.until = QueryDateTime::until_from_parts(u_d, u_t);
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
                    if let Some(f) = Filter::load(&val, db) {
                        result.t.push(f);
                    }
                }
                "p" => {
                    if let Some(f) = Filter::load(&val, db) {
                        result.p.push(f);
                    }
                }
                "l" => {
                    if let Some(f) = Filter::load(&val, db) {
                        result.l.push(f);
                    }
                }
                "pos" => {
                    result.pos = match val.as_str() {
                        "t" => Some(true),
                        "!t" => Some(false),
                        "" => None,
                        val => {
                            warn!("Bad value for \"pos\": {:?}", val);
                            None
                        }
                    }
                }
                "from" => {
                    result.since = QueryDateTime::new(
                        p::photos
                            .select(p::date)
                            .filter(p::id.eq(val.parse::<i32>()?))
                            .first(db)?,
                    )
                }
                "to" => {
                    result.until = QueryDateTime::new(
                        p::photos
                            .select(p::date)
                            .filter(p::id.eq(val.parse::<i32>()?))
                            .first(db)?,
                    )
                }
                _ => (), // ignore unknown query parameters
            }
        }
        Ok(result)
    }
    fn to_query_string(&self) -> String {
        fn or_bang(cond: bool) -> &'static str {
            if cond {
                ""
            } else {
                "!"
            }
        }
        self.t
            .iter()
            .map(|v| format!("&t={}{}", or_bang(v.inc), v.item.slug))
            .chain(
                self.l
                    .iter()
                    .map(|v| format!("&l={}{}", or_bang(v.inc), v.item.slug)),
            )
            .chain(
                self.p
                    .iter()
                    .map(|v| format!("&p={}{}", or_bang(v.inc), v.item.slug)),
            )
            .chain(self.pos.map(|v| format!("&pos={}t", or_bang(v))))
            .collect()
    }
}

#[derive(Debug, Default)]
pub struct QueryDateTime {
    val: Option<NaiveDateTime>,
}

impl QueryDateTime {
    fn new(val: Option<NaiveDateTime>) -> Self {
        QueryDateTime { val }
    }
    fn since_from_parts(date: Option<&str>, time: Option<&str>) -> Self {
        let since_midnight = NaiveTime::from_hms_milli(0, 0, 0, 0);
        QueryDateTime::new(datetime_from_parts(date, time, since_midnight))
    }
    fn until_from_parts(date: Option<&str>, time: Option<&str>) -> Self {
        let until_midnight = NaiveTime::from_hms_milli(23, 59, 59, 999);
        QueryDateTime::new(datetime_from_parts(date, time, until_midnight))
    }
    fn as_ref(&self) -> Option<&NaiveDateTime> {
        self.val.as_ref()
    }
    pub fn date_val(&self) -> QueryDateFmt {
        QueryDateFmt(self.as_ref())
    }
    pub fn time_val(&self) -> QueryTimeFmt {
        QueryTimeFmt(self.as_ref())
    }
}

pub struct QueryDateFmt<'a>(Option<&'a NaiveDateTime>);
impl<'a> templates::ToHtml for QueryDateFmt<'a> {
    fn to_html(&self, out: &mut dyn std::io::Write) -> std::io::Result<()> {
        if let Some(date) = self.0 {
            // Note: Only digits and dashes, nothing that needs escaping
            write!(out, "{}", date.format("%Y-%m-%d"))
        } else {
            Ok(())
        }
    }
}
pub struct QueryTimeFmt<'a>(Option<&'a NaiveDateTime>);
impl<'a> templates::ToHtml for QueryTimeFmt<'a> {
    fn to_html(&self, out: &mut dyn std::io::Write) -> std::io::Result<()> {
        if let Some(time) = self.0 {
            // Note: Only digits and colons, nothing that needs escaping
            write!(out, "{}", time.format("%H:%M:%S"))
        } else {
            Ok(())
        }
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
