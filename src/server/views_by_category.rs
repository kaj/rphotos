//! Handle photos by tag, person, or place.
use super::splitlist::links_by_time;
use super::{
    wrap, Context, ContextFilter, ImgRange, RenderRucte, Result, ViewError,
};
use crate::models::{Person, Photo, Place, Tag};
use crate::templates;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use warp::filters::method::get;
use warp::filters::BoxedFilter;
use warp::http::response::Builder;
use warp::path::{end, param};
use warp::query::query;
use warp::reply::Response;
use warp::Filter;

pub fn person_routes(s: ContextFilter) -> BoxedFilter<(Response,)> {
    let all = end().and(get()).and(s.clone()).then(person_all);
    let one = param()
        .and(end())
        .and(get())
        .and(query())
        .and(s)
        .then(person_one);
    all.or(one).unify().map(wrap).boxed()
}
pub fn place_routes(s: ContextFilter) -> BoxedFilter<(Response,)> {
    let all = end().and(s.clone()).and(get()).then(place_all);
    let one = param()
        .and(end())
        .and(get())
        .and(query())
        .and(s)
        .then(place_one);
    all.or(one).unify().map(wrap).boxed()
}
pub fn tag_routes(s: ContextFilter) -> BoxedFilter<(Response,)> {
    let all = end().and(s.clone()).and(get()).then(tag_all);
    let one = param()
        .and(end())
        .and(get())
        .and(query())
        .and(s)
        .then(tag_one);
    all.or(one).unify().map(wrap).boxed()
}

async fn person_all(context: Context) -> Result<Response> {
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
    let images = query
        .order(person_name)
        .load(&mut context.db().await?)
        .await?;
    Ok(Builder::new()
        .html(|o| templates::people_html(o, &context, &images))?)
}

async fn person_one(
    tslug: String,
    range: ImgRange,
    context: Context,
) -> Result<Response> {
    use crate::schema::people::dsl::{people, slug};
    let mut c = context.db().await?;
    let person = or_404q!(
        people.filter(slug.eq(tslug)).first::<Person>(&mut c).await,
        context
    );
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
    let (links, coords) = links_by_time(&context, photos, range, true).await?;
    Ok(Builder::new().html(|o| {
        templates::person_html(o, &context, &links, &coords, &person)
    })?)
}

async fn tag_all(context: Context) -> Result<Response> {
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
    let taggs = query.load(&mut context.db().await?).await?;
    Ok(Builder::new().html(|o| templates::tags_html(o, &context, &taggs))?)
}

async fn tag_one(
    tslug: String,
    range: ImgRange,
    context: Context,
) -> Result<Response> {
    use crate::schema::tags::dsl::{slug, tags};
    let tag = or_404q!(
        tags.filter(slug.eq(tslug))
            .first::<Tag>(&mut context.db().await?)
            .await,
        context
    );

    use crate::schema::photo_tags::dsl::{photo_id, photo_tags, tag_id};
    use crate::schema::photos::dsl::id;
    let photos = Photo::query(context.is_authorized()).filter(
        id.eq_any(photo_tags.select(photo_id).filter(tag_id.eq(tag.id))),
    );
    let (links, coords) = links_by_time(&context, photos, range, true).await?;
    Ok(Builder::new()
        .html(|o| templates::tag_html(o, &context, &links, &coords, &tag))?)
}

async fn place_all(context: Context) -> Result<Response> {
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
    let found = query
        .order(place_name)
        .load(&mut context.db().await?)
        .await?;
    Ok(
        Builder::new()
            .html(|o| templates::places_html(o, &context, &found))?,
    )
}

async fn place_one(
    tslug: String,
    range: ImgRange,
    context: Context,
) -> Result<Response> {
    use crate::schema::places::dsl::{places, slug};
    let place = or_404q!(
        places
            .filter(slug.eq(tslug))
            .first::<Place>(&mut context.db().await?)
            .await,
        context
    );

    use crate::schema::photo_places::dsl::{photo_id, photo_places, place_id};
    use crate::schema::photos::dsl::id;
    let photos = Photo::query(context.is_authorized()).filter(
        id.eq_any(photo_places.select(photo_id).filter(place_id.eq(place.id))),
    );
    let (links, coord) = links_by_time(&context, photos, range, true).await?;
    Ok(Builder::new().html(|o| {
        templates::place_html(o, &context, &links, &coord, &place)
    })?)
}
