use super::error::{ViewError, ViewResult};
use super::splitlist::split_to_group_links;
use super::urlstring::UrlString;
use super::{Context, RenderRucte, Result};
use crate::models::{Coord, Facet, Person, Photo, Place, Tag};
use crate::schema::photo_people::dsl as pp;
use crate::schema::photo_places::dsl as pl;
use crate::schema::photo_tags::dsl as pt;
use crate::schema::photos::dsl as p;
use crate::schema::positions::dsl as pos;
use crate::templates;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use tracing::warn;
use warp::http::response::Builder;
use warp::reply::Response;

pub async fn search(
    context: Context,
    query: Vec<(String, String)>,
) -> Result<Response> {
    let mut db = context.db().await?;
    let query = SearchQuery::load(query.try_into()?, &mut db).await?;

    let mut photos = Photo::query(context.is_authorized());
    if let Some(since) = query.since.as_ref() {
        photos = photos.filter(p::date.ge(since));
    }
    if let Some(until) = query.until.as_ref() {
        photos = photos.filter(p::date.le(until));
    }
    for tag in &query.t.include {
        let ids = pt::photo_tags
            .select(pt::photo_id)
            .filter(pt::tag_id.eq(tag.id));
        photos = photos.filter(p::id.eq_any(ids));
    }
    if !query.t.exclude.is_empty() {
        let ids = query.t.exclude.iter().map(|t| t.id).collect::<Vec<_>>();
        let ids = pt::photo_tags
            .select(pt::photo_id)
            .filter(pt::tag_id.eq_any(ids));
        photos = photos.filter(p::id.ne_all(ids));
    }
    for location in &query.l.include {
        let ids = pl::photo_places
            .select(pl::photo_id)
            .filter(pl::place_id.eq(location.id));
        photos = photos.filter(p::id.eq_any(ids));
    }
    if !query.l.exclude.is_empty() {
        let ids = query.l.exclude.iter().map(|t| t.id).collect::<Vec<_>>();
        let ids = pl::photo_places
            .select(pl::photo_id)
            .filter(pl::place_id.eq_any(ids));
        photos = photos.filter(p::id.ne_all(ids));
    }
    for person in &query.p.include {
        let ids = pp::photo_people
            .select(pp::photo_id)
            .filter(pp::person_id.eq(person.id));
        photos = photos.filter(p::id.eq_any(ids));
    }
    if !query.p.exclude.is_empty() {
        let ids = query.p.exclude.iter().map(|t| t.id).collect::<Vec<_>>();
        let ids = pp::photo_people
            .select(pp::photo_id)
            .filter(pp::person_id.eq_any(ids));
        photos = photos.filter(p::id.ne_all(ids));
    }

    if let Some(pos) = query.pos {
        let pos_ids = pos::positions.select(pos::photo_id);
        if pos {
            photos = photos.filter(p::id.eq_any(pos_ids));
        } else {
            photos = photos.filter(p::id.ne_all(pos_ids));
        }
    }

    let photos = photos
        .order((p::date.desc().nulls_last(), p::id.desc()))
        .left_join(pos::positions)
        .select((
            Photo::as_select(),
            ((pos::latitude, pos::longitude), pos::photo_id).nullable(),
        ))
        .load::<(Photo, Option<(Coord, i32)>)>(&mut db)
        .await?;

    let (photos, coords): (Vec<_>, SomeVec<_>) = photos.into_iter().unzip();
    let n = photos.len();
    let links = split_to_group_links(&photos, &query.to_base_url(), true);

    Ok(Builder::new().html(|o| {
        templates::search_html(o, &context, &query, n, &links, &coords.0)
    })?)
}

#[derive(Default, Debug)]
struct RawQuery {
    tags: InclExcl<String>,
    people: InclExcl<String>,
    locations: InclExcl<String>,
    pos: Option<bool>,
    q: String,
    since: DateTimeImg,
    until: DateTimeImg,
}

impl TryFrom<Vec<(String, String)>> for RawQuery {
    type Error = ViewError;

    fn try_from(value: Vec<(String, String)>) -> Result<Self, Self::Error> {
        let mut to = RawQuery::default();
        for (key, val) in value {
            match key.as_ref() {
                "q" => {
                    if val.contains("!pos") {
                        to.pos = Some(false);
                    } else if val.contains("pos") {
                        to.pos = Some(true);
                    }
                    to.q = val;
                }
                "t" => to.tags.add(val),
                "p" => to.people.add(val),
                "l" => to.locations.add(val),
                "pos" => {
                    to.pos = match val.as_str() {
                        "t" => Some(true),
                        "!t" => Some(false),
                        "" => None,
                        val => {
                            warn!("Bad value for \"pos\": {:?}", val);
                            None
                        }
                    }
                }
                "since_date" if !val.is_empty() => {
                    to.since.date = Some(val.parse().req("since_date")?)
                }
                "since_time" if !val.is_empty() => {
                    to.since.time = Some(val.parse().req("since_time")?)
                }
                "until_date" if !val.is_empty() => {
                    to.until.date = Some(val.parse().req("until_date")?)
                }
                "until_time" if !val.is_empty() => {
                    to.until.time = Some(val.parse().req("until_time")?)
                }
                "from" => to.since.img = Some(val.parse().req("from")?),
                "to" => to.until.img = Some(val.parse().req("to")?),
                _ => (), // ignore unknown query parameters
            }
        }
        Ok(to)
    }
}

/// A since or until time, can either be given as a date (optionally
/// with time) or as an image id to take the datetime from.
#[derive(Default, Debug)]
struct DateTimeImg {
    date: Option<NaiveDate>,
    time: Option<NaiveTime>,
    img: Option<i32>,
}

/// A `Vec` that automatically flattens an iterator of options when extended.
struct SomeVec<T>(Vec<T>);

impl<T> Default for SomeVec<T> {
    fn default() -> Self {
        SomeVec(Vec::new())
    }
}
impl<T> Extend<Option<T>> for SomeVec<T> {
    fn extend<Iter: IntoIterator<Item = Option<T>>>(&mut self, iter: Iter) {
        self.0.extend(iter.into_iter().flatten())
    }
}

#[derive(Debug, Default)]
pub struct SearchQuery {
    /// Keys
    pub t: InclExcl<Tag>,
    /// People
    pub p: InclExcl<Person>,
    /// Places (locations)
    pub l: InclExcl<Place>,
    pub since: QueryDateTime,
    pub until: QueryDateTime,
    pub pos: Option<bool>,
    /// Query (free-text, don't know what to do)
    pub q: String,
}

impl SearchQuery {
    async fn load(
        query: RawQuery,
        db: &mut AsyncPgConnection,
    ) -> Result<Self> {
        Ok(SearchQuery {
            t: InclExcl::load(query.tags, db).await?,
            p: InclExcl::load(query.people, db).await?,
            l: InclExcl::load(query.locations, db).await?,
            pos: query.pos,
            q: query.q,
            since: QueryDateTime::from_raw(
                &query.since,
                NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                db,
            )
            .await?,
            until: QueryDateTime::from_raw(
                &query.until,
                NaiveTime::from_hms_milli_opt(23, 59, 59, 999).unwrap(),
                db,
            )
            .await?,
        })
    }
    fn to_base_url(&self) -> UrlString {
        let mut result = UrlString::new("/search/");
        for (t, i) in &self.t {
            result.cond_query("t", i, &t.slug);
        }
        for (l, i) in &self.l {
            result.cond_query("l", i, &l.slug);
        }
        for (p, i) in &self.p {
            result.cond_query("p", i, &p.slug);
        }
        for i in &self.pos {
            result.cond_query("pos", *i, "t");
        }
        result
    }
}

#[derive(Debug)]
pub struct InclExcl<T> {
    include: Vec<T>,
    exclude: Vec<T>,
}
impl<T> Default for InclExcl<T> {
    fn default() -> Self {
        InclExcl {
            include: Vec::new(),
            exclude: Vec::new(),
        }
    }
}
impl<'a, T> IntoIterator for &'a InclExcl<T> {
    type Item = (&'a T, bool);
    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(
            self.include
                .iter()
                .map(|t| (t, true))
                .chain(self.exclude.iter().map(|t| (t, false))),
        )
    }
}

impl InclExcl<String> {
    // TODO: Check that data (after optional bang) is a valid slug.  Return result.
    fn add(&mut self, data: String) {
        match data.strip_prefix('!') {
            Some(val) => self.exclude.push(val.into()),
            None => self.include.push(data),
        };
    }
}

impl<T: Facet> InclExcl<T> {
    async fn load(
        val: InclExcl<String>,
        db: &mut AsyncPgConnection,
    ) -> Result<Self> {
        Ok(InclExcl {
            include: if val.include.is_empty() {
                Vec::new()
            } else {
                T::load_slugs(&val.include, db).await?
            },
            exclude: if val.exclude.is_empty() {
                Vec::new()
            } else {
                T::load_slugs(&val.exclude, db).await?
            },
        })
    }
}

#[derive(Debug, Default)]
pub struct QueryDateTime {
    val: Option<NaiveDateTime>,
}

impl QueryDateTime {
    async fn from_raw(
        raw: &DateTimeImg,
        def_time: NaiveTime,
        db: &mut AsyncPgConnection,
    ) -> Result<Self> {
        let val = if let Some(img_id) = raw.img {
            p::photos
                .select(p::date)
                .filter(p::id.eq(img_id))
                .first(db)
                .await?
        } else {
            None
        };

        Ok(QueryDateTime {
            val: val.or_else(|| {
                raw.date
                    .map(|date| date.and_time(raw.time.unwrap_or(def_time)))
            }),
        })
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
