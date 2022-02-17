use super::{wrap, Context, Result};
use crate::schema::people::dsl as h; // h as in human
use crate::schema::photo_people::dsl as pp;
use crate::schema::photos::dsl as p;
use crate::schema::places::dsl as l;
use crate::schema::tags::dsl as t;
use diesel::prelude::*;
use diesel::sql_types::Text;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use warp::filters::method::get;
use warp::filters::BoxedFilter;
use warp::path::{end, path};
use warp::query::query;
use warp::reply::{json, Json, Response};
use warp::Filter;

pub fn routes(s: BoxedFilter<(Context,)>) -> BoxedFilter<(Response,)> {
    end()
        .and(get())
        .and(s.clone())
        .and(query())
        .map(auto_complete_any)
        .or(path("tag")
            .and(get())
            .and(s.clone())
            .and(query())
            .map(auto_complete_tag))
        .unify()
        .or(path("person")
            .and(get())
            .and(s)
            .and(query())
            .map(auto_complete_person))
        .unify()
        .map(wrap)
        .boxed()
}

sql_function!(fn lower(string: Text) -> Text);
sql_function!(fn strpos(string: Text, substring: Text) -> Integer);

pub fn auto_complete_any(context: Context, query: AcQ) -> Result<Json> {
    let qq = query.q.to_lowercase();

    let tpos = strpos(lower(t::tag_name), &qq);
    let query = t::tags
        .select((t::tag_name, t::slug, tpos))
        .filter(tpos.gt(0))
        .into_boxed();
    let query = if context.is_authorized() {
        query
    } else {
        use crate::schema::photo_tags::dsl as tp;
        query.filter(t::id.eq_any(tp::photo_tags.select(tp::tag_id).filter(
            tp::photo_id.eq_any(p::photos.select(p::id).filter(p::is_public)),
        )))
    };
    let db = context.db()?;
    let mut tags = query
        .order((tpos, t::tag_name))
        .limit(10)
        .load::<(String, String, i32)>(&db)?
        .into_iter()
        .map(|(t, s, p)| (SearchTag { k: 't', t, s }, p))
        .collect::<Vec<_>>();
    tags.extend({
        let ppos = strpos(lower(h::person_name), &qq);
        let query = h::people
            .select((h::person_name, h::slug, ppos))
            .filter(ppos.gt(0))
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
            .order((ppos, h::person_name))
            .limit(10)
            .load::<(String, String, i32)>(&db)?
            .into_iter()
            .map(|(t, s, p)| (SearchTag { k: 'p', t, s }, p))
    });
    tags.extend({
        let lpos = strpos(lower(l::place_name), &qq);
        let query = l::places
            .select((l::place_name, l::slug, lpos))
            .filter(lpos.gt(0))
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
            .order((lpos, l::place_name))
            .limit(10)
            .load::<(String, String, i32)>(&db)?
            .into_iter()
            .map(|(t, s, p)| (SearchTag { k: 'l', t, s }, p))
    });
    tags.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    Ok(json(&tags.iter().map(|(t, _)| t).collect::<Vec<_>>()))
}

pub fn auto_complete_tag(context: Context, query: AcQ) -> Result<Json> {
    use crate::schema::tags::dsl::{tag_name, tags};
    let tpos = strpos(lower(tag_name), query.q.to_lowercase());
    let q = tags
        .select(tag_name)
        .filter((&tpos).gt(0))
        .order((&tpos, tag_name))
        .limit(10);
    Ok(json(&q.load::<String>(&context.db()?)?))
}

pub fn auto_complete_person(context: Context, query: AcQ) -> Result<Json> {
    use crate::schema::people::dsl::{people, person_name};
    let mpos = strpos(lower(person_name), query.q.to_lowercase());
    let q = people
        .select(person_name)
        .filter((&mpos).gt(0))
        .order((&mpos, person_name))
        .limit(10);
    Ok(json(&q.load::<String>(&context.db()?)?))
}

#[derive(Deserialize)]
pub struct AcQ {
    pub q: String,
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
        self.t.cmp(&o.t)
    }
}
impl PartialOrd for SearchTag {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
