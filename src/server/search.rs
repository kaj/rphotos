use super::splitlist::{get_positions, split_to_group_links};
use super::urlstring::UrlString;
use super::{Context, RenderRucte};
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

    let c = context.db().unwrap();
    let photos = photos
        .order((p::date.desc().nulls_last(), p::id.desc()))
        .load(&c)
        .unwrap();

    let coords = get_positions(&photos, &c);
    let links = split_to_group_links(&photos, &query.to_base_url(), true);

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
                    result.since = QueryDateTime::from_img(val.parse()?, db)?;
                }
                "to" => {
                    result.until = QueryDateTime::from_img(val.parse()?, db)?;
                }
                _ => (), // ignore unknown query parameters
            }
        }
        Ok(result)
    }
    fn to_base_url(&self) -> UrlString {
        let mut result = UrlString::new("/search/");
        for i in &self.t {
            result.cond_query("t", i.inc, &i.item.slug);
        }
        for i in &self.l {
            result.cond_query("l", i.inc, &i.item.slug);
        }
        for i in &self.p {
            result.cond_query("p", i.inc, &i.item.slug);
        }
        for i in &self.pos {
            result.cond_query("pos", *i, "t");
        }
        result
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
    fn from_img(photo_id: i32, db: &PgConnection) -> Result<Self, Error> {
        Ok(QueryDateTime::new(
            p::photos
                .select(p::date)
                .filter(p::id.eq(photo_id))
                .first(db)?,
        ))
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
