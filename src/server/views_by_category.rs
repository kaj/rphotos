//! Handle photos by tag, person, or place.
use super::splitlist::links_by_time;
use super::{
    Context, ContextFilter, ImgRange, RenderRucte, Result, ViewError, wrap,
};
use crate::models::{Person, Photo, Place, Tag};
use crate::schema::people::dsl as h;
use crate::schema::photo_people::dsl as pp;
use crate::schema::photo_places::dsl as pl;
use crate::schema::photo_tags::dsl as pt;
use crate::schema::photos::dsl as p;
use crate::schema::places::dsl as l;
use crate::schema::tags::dsl as t;
use crate::templates;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use warp::Filter;
use warp::filters::BoxedFilter;
use warp::filters::method::get;
use warp::http::response::Builder;
use warp::path::{end, param};
use warp::query::query;
use warp::reply::Response;

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
    let query = h::people.into_boxed();
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
    let images = query
        .order(h::person_name)
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
    let mut c = context.db().await?;
    let person = or_404q!(
        h::people
            .filter(h::slug.eq(tslug))
            .first::<Person>(&mut c)
            .await,
        context
    );
    let photos = Photo::query(context.is_authorized()).filter(
        p::id.eq_any(
            pp::photo_people
                .select(pp::photo_id)
                .filter(pp::person_id.eq(person.id)),
        ),
    );
    let (links, coords) = links_by_time(&context, photos, range, true).await?;
    Ok(Builder::new().html(|o| {
        templates::person_html(o, &context, &links, &coords, &person)
    })?)
}

async fn tag_all(context: Context) -> Result<Response> {
    let query = t::tags.order(t::tag_name).into_boxed();
    let query = if context.is_authorized() {
        query
    } else {
        query.filter(t::id.eq_any(pt::photo_tags.select(pt::tag_id).filter(
            pt::photo_id.eq_any(p::photos.select(p::id).filter(p::is_public)),
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
    let tag = or_404q!(
        t::tags
            .filter(t::slug.eq(tslug))
            .first::<Tag>(&mut context.db().await?)
            .await,
        context
    );

    let photos = Photo::query(context.is_authorized()).filter(
        p::id.eq_any(
            pt::photo_tags
                .select(pt::photo_id)
                .filter(pt::tag_id.eq(tag.id)),
        ),
    );
    let (links, coords) = links_by_time(&context, photos, range, true).await?;
    Ok(Builder::new()
        .html(|o| templates::tag_html(o, &context, &links, &coords, &tag))?)
}

async fn place_all(context: Context) -> Result<Response> {
    let query = l::places.into_boxed();
    let query = if context.is_authorized() {
        query
    } else {
        query.filter(
            l::id.eq_any(
                pl::photo_places.select(pl::place_id).filter(
                    pl::photo_id
                        .eq_any(p::photos.select(p::id).filter(p::is_public)),
                ),
            ),
        )
    };
    let found = query
        .order(l::place_name)
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
    let place = or_404q!(
        l::places
            .filter(l::slug.eq(tslug))
            .first::<Place>(&mut context.db().await?)
            .await,
        context
    );

    let photos = Photo::query(context.is_authorized()).filter(
        p::id.eq_any(
            pl::photo_places
                .select(pl::photo_id)
                .filter(pl::place_id.eq(place.id)),
        ),
    );
    let (links, coord) = links_by_time(&context, photos, range, true).await?;
    Ok(Builder::new().html(|o| {
        templates::place_html(o, &context, &links, &coord, &place)
    })?)
}
