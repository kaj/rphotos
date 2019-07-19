//! Handle photos by tag, person, or place.
use super::render_ructe::RenderRucte;
use super::{links_by_time, not_found, Context, ImgRange};
use crate::models::{Person, Photo, Place, Tag};
use crate::templates;
use diesel::prelude::*;
use serde::Deserialize;
use warp::http::Response;
use warp::{reply, Reply};

pub fn person_all(context: Context) -> Response<Vec<u8>> {
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
    Response::builder().html(|o| {
        templates::people(
            o,
            &context,
            &query
                .order(person_name)
                .load(context.db())
                .expect("list people"),
        )
    })
}

pub fn person_one(
    context: Context,
    tslug: String,
    range: ImgRange,
) -> Response<Vec<u8>> {
    use crate::schema::people::dsl::{people, slug};
    let c = context.db();
    if let Ok(person) = people.filter(slug.eq(tslug)).first::<Person>(c) {
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
        Response::builder()
            .html(|o| templates::person(o, &context, &links, &coords, &person))
    } else {
        not_found(&context)
    }
}

pub fn tag_all(context: Context) -> Response<Vec<u8>> {
    use crate::schema::tags::dsl::{id, tag_name, tags};
    let query = tags.into_boxed();
    let query = if context.is_authorized() {
        query
    } else {
        use crate::schema::photo_tags::dsl as tp;
        use crate::schema::photos::dsl as p;
        query.filter(id.eq_any(tp::photo_tags.select(tp::tag_id).filter(
            tp::photo_id.eq_any(p::photos.select(p::id).filter(p::is_public)),
        )))
    };
    Response::builder().html(|o| {
        templates::tags(
            o,
            &context,
            &query.order(tag_name).load(context.db()).expect("List tags"),
        )
    })
}

pub fn tag_one(
    context: Context,
    tslug: String,
    range: ImgRange,
) -> Response<Vec<u8>> {
    use crate::schema::tags::dsl::{slug, tags};
    if let Ok(tag) = tags.filter(slug.eq(tslug)).first::<Tag>(context.db()) {
        use crate::schema::photo_tags::dsl::{photo_id, photo_tags, tag_id};
        use crate::schema::photos::dsl::id;
        let photos = Photo::query(context.is_authorized()).filter(
            id.eq_any(photo_tags.select(photo_id).filter(tag_id.eq(tag.id))),
        );
        let (links, coords) = links_by_time(&context, photos, range, true);
        Response::builder()
            .html(|o| templates::tag(o, &context, &links, &coords, &tag))
    } else {
        not_found(&context)
    }
}

pub fn place_all(context: Context) -> Response<Vec<u8>> {
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
    Response::builder().html(|o| {
        templates::places(
            o,
            &context,
            &query
                .order(place_name)
                .load(context.db())
                .expect("List places"),
        )
    })
}

pub fn place_one(
    context: Context,
    tslug: String,
    range: ImgRange,
) -> Response<Vec<u8>> {
    use crate::schema::places::dsl::{places, slug};
    if let Ok(place) =
        places.filter(slug.eq(tslug)).first::<Place>(context.db())
    {
        use crate::schema::photo_places::dsl::{
            photo_id, photo_places, place_id,
        };
        use crate::schema::photos::dsl::id;
        let photos = Photo::query(context.is_authorized()).filter(id.eq_any(
            photo_places.select(photo_id).filter(place_id.eq(place.id)),
        ));
        let (links, coord) = links_by_time(&context, photos, range, true);
        Response::builder()
            .html(|o| templates::place(o, &context, &links, &coord, &place))
    } else {
        not_found(&context)
    }
}

pub fn auto_complete_tag(context: Context, query: AcQ) -> impl Reply {
    use crate::schema::tags::dsl::{tag_name, tags};
    let q = tags
        .select(tag_name)
        .filter(tag_name.ilike(query.q + "%"))
        .order(tag_name)
        .limit(10);
    reply::json(&q.load::<String>(context.db()).unwrap())
}

pub fn auto_complete_person(context: Context, query: AcQ) -> impl Reply {
    use crate::schema::people::dsl::{people, person_name};
    let q = people
        .select(person_name)
        .filter(person_name.ilike(query.q + "%"))
        .order(person_name)
        .limit(10);
    reply::json(&q.load::<String>(context.db()).unwrap())
}

#[derive(Deserialize)]
pub struct AcQ {
    pub q: String,
}
