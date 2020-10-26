//! Handle photos by tag, person, or place.
use super::splitlist::links_by_time;
use super::{not_found, Context, ContextFilter, ImgRange, RenderRucte};
use crate::models::{Person, Photo, Place, Tag};
use crate::templates;
use diesel::prelude::*;
use warp::filters::method::get;
use warp::filters::BoxedFilter;
use warp::http::response::Builder;
use warp::path::{end, param};
use warp::query::query;
use warp::reply::Response;
use warp::{Filter, Reply};

pub fn person_routes(s: ContextFilter) -> BoxedFilter<(impl Reply,)> {
    end()
        .and(s.clone())
        .and(get())
        .map(person_all)
        .or(s
            .and(param())
            .and(end())
            .and(get())
            .and(query())
            .map(person_one))
        .boxed()
}
pub fn place_routes(s: ContextFilter) -> BoxedFilter<(impl Reply,)> {
    end()
        .and(s.clone())
        .and(get())
        .map(place_all)
        .or(s
            .and(param())
            .and(end())
            .and(get())
            .and(query())
            .map(place_one))
        .boxed()
}
pub fn tag_routes(s: ContextFilter) -> BoxedFilter<(impl Reply,)> {
    end()
        .and(s.clone())
        .and(get())
        .map(tag_all)
        .or(s
            .and(param())
            .and(end())
            .and(get())
            .and(query())
            .map(tag_one))
        .boxed()
}

fn person_all(context: Context) -> Response {
    use crate::schema::people::dsl::{id, people, person_name};
    let query = people.into_boxed();
    let query = if context.is_authorized() {
        query
    } else {
        use crate::schema::photo_people::dsl as pp;
        use crate::schema::photos::dsl as p;
        query.filter(id.eq_any(pp::photo_people.select(pp::person_id).filter(
            pp::photo_id.eq_any(p::photos.select(p::id).filter(p::is_public)),
        )))
    };
    Builder::new()
        .html(|o| {
            templates::people(
                o,
                &context,
                &query
                    .order(person_name)
                    .load(&context.db().unwrap())
                    .expect("list people"),
            )
        })
        .unwrap()
}

fn person_one(context: Context, tslug: String, range: ImgRange) -> Response {
    use crate::schema::people::dsl::{people, slug};
    let c = context.db().unwrap();
    if let Ok(person) = people.filter(slug.eq(tslug)).first::<Person>(&c) {
        use crate::schema::photo_people::dsl::{
            person_id, photo_id, photo_people,
        };
        use crate::schema::photos::dsl::id;
        let photos = Photo::query(context.is_authorized()).filter(
            id.eq_any(
                photo_people
                    .select(photo_id)
                    .filter(person_id.eq(person.id)),
            ),
        );
        let (links, coords) = links_by_time(&context, photos, range, true);
        Builder::new()
            .html(|o| templates::person(o, &context, &links, &coords, &person))
            .unwrap()
    } else {
        not_found(&context)
    }
}

fn tag_all(context: Context) -> Response {
    use crate::schema::tags::dsl::{id, tag_name, tags};
    let query = tags.order(tag_name).into_boxed();
    let query = if context.is_authorized() {
        query
    } else {
        use crate::schema::photo_tags::dsl as tp;
        use crate::schema::photos::dsl as p;
        query.filter(id.eq_any(tp::photo_tags.select(tp::tag_id).filter(
            tp::photo_id.eq_any(p::photos.select(p::id).filter(p::is_public)),
        )))
    };
    Builder::new()
        .html(|o| {
            templates::tags(
                o,
                &context,
                &query.load(&context.db().unwrap()).expect("List tags"),
            )
        })
        .unwrap()
}

fn tag_one(context: Context, tslug: String, range: ImgRange) -> Response {
    use crate::schema::tags::dsl::{slug, tags};
    if let Ok(tag) = tags
        .filter(slug.eq(tslug))
        .first::<Tag>(&context.db().unwrap())
    {
        use crate::schema::photo_tags::dsl::{photo_id, photo_tags, tag_id};
        use crate::schema::photos::dsl::id;
        let photos = Photo::query(context.is_authorized()).filter(
            id.eq_any(photo_tags.select(photo_id).filter(tag_id.eq(tag.id))),
        );
        let (links, coords) = links_by_time(&context, photos, range, true);
        Builder::new()
            .html(|o| templates::tag(o, &context, &links, &coords, &tag))
            .unwrap()
    } else {
        not_found(&context)
    }
}

fn place_all(context: Context) -> Response {
    use crate::schema::places::dsl::{id, place_name, places};
    let query = places.into_boxed();
    let query = if context.is_authorized() {
        query
    } else {
        use crate::schema::photo_places::dsl as pp;
        use crate::schema::photos::dsl as p;
        query.filter(id.eq_any(pp::photo_places.select(pp::place_id).filter(
            pp::photo_id.eq_any(p::photos.select(p::id).filter(p::is_public)),
        )))
    };
    Builder::new()
        .html(|o| {
            templates::places(
                o,
                &context,
                &query
                    .order(place_name)
                    .load(&context.db().unwrap())
                    .expect("List places"),
            )
        })
        .unwrap()
}

fn place_one(context: Context, tslug: String, range: ImgRange) -> Response {
    use crate::schema::places::dsl::{places, slug};
    if let Ok(place) = places
        .filter(slug.eq(tslug))
        .first::<Place>(&context.db().unwrap())
    {
        use crate::schema::photo_places::dsl::{
            photo_id, photo_places, place_id,
        };
        use crate::schema::photos::dsl::id;
        let photos = Photo::query(context.is_authorized()).filter(id.eq_any(
            photo_places.select(photo_id).filter(place_id.eq(place.id)),
        ));
        let (links, coord) = links_by_time(&context, photos, range, true);
        Builder::new()
            .html(|o| templates::place(o, &context, &links, &coord, &place))
            .unwrap()
    } else {
        not_found(&context)
    }
}
