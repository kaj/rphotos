use super::{Context, Result, wrap};
use crate::schema::people::dsl as h; // h as in human
use crate::schema::photo_people::dsl as pp;
use crate::schema::photo_places::dsl as lp;
use crate::schema::photo_tags::dsl as tp;
use crate::schema::photos::dsl as p;
use crate::schema::places::dsl as l;
use crate::schema::tags::dsl as t;
use diesel::prelude::*;
use diesel::sql_types::Text;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt::Display;
use warp::Filter;
use warp::filters::BoxedFilter;
use warp::filters::method::get;
use warp::path::{end, path};
use warp::query::query;
use warp::reply::{Json, Response, json};

pub fn routes(s: BoxedFilter<(Context,)>) -> BoxedFilter<(Response,)> {
    let egs = end().and(get()).and(s);
    let any = egs.clone().and(query()).then(list_any);
    let tag = path("tag").and(egs.clone()).and(query()).then(list_tags);
    let person = path("person").and(egs).and(query()).then(list_people);
    any.or(tag).unify().or(person).unify().map(wrap).boxed()
}

async fn list_any(context: Context, term: AcQ) -> Result<Json> {
    let mut tags = select_tags(&context, &term)
        .await?
        .into_iter()
        .map(|(t, s, p)| (SearchTag { k: 't', t, s }, p))
        .chain({
            select_people(&context, &term)
                .await?
                .into_iter()
                .map(|(t, s, p)| (SearchTag { k: 'p', t, s }, p))
        })
        .chain({
            select_places(&context, &term)
                .await?
                .into_iter()
                .map(|(t, s, p)| (SearchTag { k: 'l', t, s }, p))
        })
        .collect::<Vec<_>>();
    tags.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    Ok(json(&tags.iter().map(|(t, _)| t).collect::<Vec<_>>()))
}

async fn list_tags(context: Context, query: AcQ) -> Result<Json> {
    Ok(json(&names(select_tags(&context, &query).await?)))
}

async fn list_people(context: Context, query: AcQ) -> Result<Json> {
    Ok(json(&names(select_people(&context, &query).await?)))
}

fn names(data: Vec<NameSlugScore>) -> Vec<String> {
    data.into_iter().map(|(name, _, _)| name).collect()
}

/// A `q` query argument which must not contain the null character.
///
/// Diesel escapes quotes and other dangerous chars, but null must be
/// avoided.
#[derive(Deserialize)]
#[serde(try_from = "RawAcQ")]
struct AcQ {
    q: String,
}

#[derive(Deserialize)]
struct RawAcQ {
    q: String,
}

impl TryFrom<RawAcQ> for AcQ {
    type Error = NullInQuery;

    fn try_from(q: RawAcQ) -> Result<Self, Self::Error> {
        if q.q.as_bytes().contains(&0) {
            Err(NullInQuery)
        } else {
            Ok(AcQ {
                q: q.q.to_lowercase(),
            })
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct NullInQuery;
impl Display for NullInQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Query must not contain null")
    }
}

#[derive(Debug, Serialize, Eq, PartialEq)]
struct SearchTag {
    /// Kind (may be "p" for person, "t" for tag, "l" for location).
    k: char,
    /// Title of the the tag
    t: String,
    /// Slug
    s: String,
}

impl Ord for SearchTag {
    fn cmp(&self, o: &Self) -> Ordering {
        self.t.cmp(&o.t).then_with(|| self.k.cmp(&o.k))
    }
}
impl PartialOrd for SearchTag {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

#[cfg(test)]
mod tests {
    use super::AcQ;

    fn parse(data: &[u8]) -> Result<String, String> {
        serde_urlencoded::from_bytes::<AcQ>(data)
            .map(|q| q.q)
            .map_err(|e| e.to_string())
    }

    #[test]
    fn query_good() {
        assert_eq!(parse(b"q=FooBar").as_deref(), Ok("foobar"));
    }
    #[test]
    fn query_ugly() {
        assert_eq!(parse(b"q=%22%27%60%2C%C3%9E").as_deref(), Ok("\"'`,þ"));
    }
    #[test]
    fn query_bad_contains_null() {
        assert_eq!(
            parse(b"q=Foo%00Bar"),
            Err("Query must not contain null".into()),
        );
    }
}

define_sql_function!(fn lower(string: Text) -> Text);
define_sql_function!(fn strpos(string: Text, substring: Text) -> Integer);

type NameSlugScore = (String, String, i32);

async fn select_tags(
    context: &Context,
    term: &AcQ,
) -> Result<Vec<NameSlugScore>> {
    let tpos = strpos(lower(t::tag_name), &term.q);
    let query = t::tags
        .select((t::tag_name, t::slug, tpos))
        .filter(tpos.gt(0))
        .into_boxed();
    let query = if context.is_authorized() {
        query
    } else {
        query.filter(t::id.eq_any(tp::photo_tags.select(tp::tag_id).filter(
            tp::photo_id.eq_any(p::photos.select(p::id).filter(p::is_public)),
        )))
    };
    let mut db = context.db().await?;
    Ok(query
        .order((tpos, t::tag_name))
        .limit(10)
        .load(&mut db)
        .await?)
}

async fn select_people(
    context: &Context,
    term: &AcQ,
) -> Result<Vec<NameSlugScore>> {
    let ppos = strpos(lower(h::person_name), &term.q);
    let query = h::people
        .select((h::person_name, h::slug, ppos))
        .filter(ppos.gt(0))
        .into_boxed();
    let query = if context.is_authorized() {
        query
    } else {
        query.filter(
            h::id.eq_any(
                pp::photo_people.select(pp::person_id).filter(
                    pp::photo_id
                        .eq_any(p::photos.select(p::id).filter(p::is_public)),
                ),
            ),
        )
    };
    let mut db = context.db().await?;
    Ok(query
        .order((ppos, h::person_name))
        .limit(10)
        .load(&mut db)
        .await?)
}

async fn select_places(
    context: &Context,
    term: &AcQ,
) -> Result<Vec<NameSlugScore>> {
    let lpos = strpos(lower(l::place_name), &term.q);
    let query = l::places
        .select((l::place_name, l::slug, lpos))
        .filter(lpos.gt(0))
        .into_boxed();
    let query = if context.is_authorized() {
        query
    } else {
        query.filter(
            l::id.eq_any(
                lp::photo_places.select(lp::place_id).filter(
                    lp::photo_id
                        .eq_any(p::photos.select(p::id).filter(p::is_public)),
                ),
            ),
        )
    };
    let mut db = context.db().await?;
    Ok(query
        .order((lpos, l::place_name))
        .limit(10)
        .load(&mut db)
        .await?)
}
