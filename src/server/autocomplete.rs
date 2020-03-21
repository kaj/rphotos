use super::Context;
use crate::schema::people::dsl as h; // h as in human
use crate::schema::photo_people::dsl as pp;
use crate::schema::photos::dsl as p;
use crate::schema::places::dsl as l;
use crate::schema::tags::dsl as t;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use warp::filters::method::get;
use warp::filters::BoxedFilter;
use warp::path::{end, path};
use warp::query::query;
use warp::{reply, Filter, Reply};

pub fn routes(s: BoxedFilter<(Context,)>) -> BoxedFilter<(impl Reply,)> {
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
        .or(path("person")
            .and(get())
            .and(s)
            .and(query())
            .map(auto_complete_person))
        .boxed()
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

pub fn auto_complete_tag(context: Context, query: AcQ) -> impl Reply {
    use crate::schema::tags::dsl::{tag_name, tags};
    let q = tags
        .select(tag_name)
        .filter(tag_name.ilike(query.q + "%"))
        .order(tag_name)
        .limit(10);
    reply::json(&q.load::<String>(&context.db().unwrap()).unwrap())
}

pub fn auto_complete_person(context: Context, query: AcQ) -> impl Reply {
    use crate::schema::people::dsl::{people, person_name};
    let q = people
        .select(person_name)
        .filter(person_name.ilike(query.q + "%"))
        .order(person_name)
        .limit(10);
    reply::json(&q.load::<String>(&context.db().unwrap()).unwrap())
}

#[derive(Deserialize)]
pub struct AcQ {
    pub q: String,
}

#[derive(Debug, Serialize)]
struct SearchTag {
    /// Kind (may be "p" for person, "t" for tag, "l" for location).
    k: char,
    /// Title of the the tag
    t: String,
    /// Slug
    s: String,
}
