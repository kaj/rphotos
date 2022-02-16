//! Handle photos by tag, person, or place.
use super::splitlist::links_by_time;
use super::{
    wrap, Context, ContextFilter, ImgRange, RenderRucte, Result, ViewError,
};
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
        .map(wrap)
        .or(s
            .and(param())
            .and(end())
            .and(get())
            .and(query())
            .map(person_one)
            .map(wrap))
        .boxed()
}
pub fn place_routes(s: ContextFilter) -> BoxedFilter<(impl Reply,)> {
    end()
        .and(s.clone())
        .and(get())
        .map(place_all)
        .map(wrap)
        .or(s
            .and(param())
            .and(end())
            .and(get())
            .and(query())
            .map(place_one)
            .map(wrap))
        .boxed()
}
pub fn tag_routes(s: ContextFilter) -> BoxedFilter<(impl Reply,)> {
    end()
        .and(s.clone())
        .and(get())
        .map(tag_all)
        .map(wrap)
        .or(s
            .and(param())
            .and(end())
            .and(get())
            .and(query())
            .map(tag_one)
            .map(wrap))
        .boxed()
}

fn person_all(context: Context) -> Result<Response> {
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
    let images = query.order(person_name).load(&context.db()?)?;
    Ok(Builder::new().html(|o| templates::people(o, &context, &images))?)
}

fn person_one(
    context: Context,
    tslug: String,
    range: ImgRange,
) -> Result<Response> {
    use crate::schema::people::dsl::{people, slug};
    let c = context.db()?;
    let person =
        or_404q!(people.filter(slug.eq(tslug)).first::<Person>(&c), context);
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
    let (links, coords) = links_by_time(&context, photos, range, true)?;
    Ok(Builder::new()
        .html(|o| templates::person(o, &context, &links, &coords, &person))?)
}

fn tag_all(context: Context) -> Result<Response> {
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
    let taggs = query.load(&context.db()?)?;
    Ok(Builder::new().html(|o| templates::tags(o, &context, &taggs))?)
}

fn tag_one(
    context: Context,
    tslug: String,
    range: ImgRange,
) -> Result<Response> {
    use crate::schema::tags::dsl::{slug, tags};
    let tag = or_404q!(
        tags.filter(slug.eq(tslug)).first::<Tag>(&context.db()?),
        context
    );

    use crate::schema::photo_tags::dsl::{photo_id, photo_tags, tag_id};
    use crate::schema::photos::dsl::id;
    let photos = Photo::query(context.is_authorized()).filter(
        id.eq_any(photo_tags.select(photo_id).filter(tag_id.eq(tag.id))),
    );
    let (links, coords) = links_by_time(&context, photos, range, true)?;
    Ok(Builder::new()
        .html(|o| templates::tag(o, &context, &links, &coords, &tag))?)
}

fn place_all(context: Context) -> Result<Response> {
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
    let found = query.order(place_name).load(&context.db()?)?;
    Ok(Builder::new().html(|o| templates::places(o, &context, &found))?)
}

fn place_one(
    context: Context,
    tslug: String,
    range: ImgRange,
) -> Result<Response> {
    use crate::schema::places::dsl::{places, slug};
    let place = or_404q!(
        places.filter(slug.eq(tslug)).first::<Place>(&context.db()?),
        context
    );

    use crate::schema::photo_places::dsl::{photo_id, photo_places, place_id};
    use crate::schema::photos::dsl::id;
    let photos = Photo::query(context.is_authorized()).filter(
        id.eq_any(photo_places.select(photo_id).filter(place_id.eq(place.id))),
    );
    let (links, coord) = links_by_time(&context, photos, range, true)?;
    Ok(Builder::new()
        .html(|o| templates::place(o, &context, &links, &coord, &place))?)
}
