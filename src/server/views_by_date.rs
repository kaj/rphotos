use super::splitlist::links_by_time;
use super::{
    redirect_to_img, wrap, Context, ContextFilter, ImgRange, Link, PhotoLink,
    Result, ViewError,
};
use crate::models::{Photo, SizeTag};
use crate::schema::photos::dsl as p;
use crate::schema::positions::dsl as ps;
use crate::templates::{self, RenderRucte};
use chrono::naive::{NaiveDate, NaiveDateTime};
use chrono::{Datelike, Duration, Local};
use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::sql_types::{Bool, Nullable, Timestamp};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use serde::Deserialize;
use warp::filters::BoxedFilter;
use warp::http::response::Builder;
use warp::path::{end, param};
use warp::query::query;
use warp::reply::Response;
use warp::{get, path, Filter};

pub fn routes(s: ContextFilter) -> BoxedFilter<(Response,)> {
    let s = move || s.clone();
    let root = end().and(get()).and(s()).then(all_years);
    let nodate = path("0").and(end()).and(get()).and(s()).then(all_null_date);
    let year = param().and(end()).and(get()).and(s()).then(months_in_year);
    let month = param()
        .and(param())
        .and(end())
        .and(get())
        .and(s())
        .then(days_in_month);
    let day = param()
        .and(param())
        .and(param())
        .and(end())
        .and(query())
        .and(get())
        .and(s())
        .then(all_for_day);

    let this = path("thisday")
        .and(end())
        .and(get())
        .and(s())
        .then(on_this_day);
    let next = path("next")
        .and(end())
        .and(get())
        .and(s())
        .and(query())
        .then(next_image);
    let prev = path("prev")
        .and(end())
        .and(get())
        .and(s())
        .and(query())
        .then(prev_image);

    root.or(nodate)
        .unify()
        .or(year)
        .unify()
        .or(month)
        .unify()
        .or(day)
        .unify()
        .or(this)
        .unify()
        .or(next)
        .unify()
        .or(prev)
        .unify()
        .map(wrap)
        .boxed()
}

sql_function! {
    #[aggregate]
    fn year_of_timestamp(date: Nullable<Timestamp>) -> Nullable<SmallInt>
}
sql_function! {
    #[aggregate]
    fn month_of_timestamp(date: Nullable<Timestamp>) -> Nullable<SmallInt>
}
sql_function! {
    #[aggregate]
    fn day_of_timestamp(date: Nullable<Timestamp>) -> Nullable<SmallInt>
}

mod filter {
    use diesel::sql_function;
    use diesel::sql_types::{Nullable, Timestamp};

    sql_function! {
        fn year_of_timestamp(date: Nullable<Timestamp>) -> Nullable<SmallInt>
    }
    sql_function! {
        fn month_of_timestamp(date: Nullable<Timestamp>) -> Nullable<SmallInt>
    }
    sql_function! {
        fn day_of_timestamp(date: Nullable<Timestamp>) -> Nullable<SmallInt>
    }
}
async fn all_years(context: Context) -> Result<Response> {
    let mut db = context.db().await?;
    let y = year_of_timestamp(p::date);
    let groups_in = p::photos
        .filter(p::path.not_like("%.CR2"))
        .filter(p::path.not_like("%.dng"))
        .filter(p::is_public.or::<_, Bool>(context.is_authorized()))
        .select((y, count_star()))
        .group_by(y)
        .order(y.desc().nulls_last())
        .load::<(Option<i16>, i64)>(&mut db)
        .await?;
    let mut groups = Vec::with_capacity(groups_in.len());
    for (year, count) in groups_in {
        let year: Option<i32> = year.map(Into::into);
        let q = Photo::query(context.is_authorized())
            .order((p::grade.desc().nulls_last(), p::date.asc()))
            .limit(1);
        let photo = if let Some(year) = year {
            q.filter(p::date.ge(start_of_year(year)))
                .filter(p::date.lt(start_of_year(year + 1)))
        } else {
            q.filter(p::date.is_null())
        };
        let photo = photo.first::<Photo>(&mut db).await?;
        groups.push(PhotoLink {
            title: Some(
                year.map_or_else(|| "-".to_string(), |y| y.to_string()),
            ),
            href: format!("/{}/", year.unwrap_or(0)),
            lable: Some(format!("{count} images")),
            id: photo.id,
            size: photo.get_size(SizeTag::Small),
        });
    }

    Ok(Builder::new().html(|o| {
        templates::index_html(o, &context, "All photos", &[], &groups, &[])
    })?)
}

async fn months_in_year(year: i32, context: Context) -> Result<Response> {
    let title: String = format!("Photos from {year}");
    let mut db = context.db().await?;
    let m = month_of_timestamp(p::date);
    let groups_in = p::photos
        .filter(p::path.not_like("%.CR2"))
        .filter(p::path.not_like("%.dng"))
        .filter(p::is_public.or::<_, Bool>(context.is_authorized()))
        .filter(p::date.ge(start_of_year(year)))
        .filter(p::date.lt(start_of_year(year + 1)))
        .select((m, count_star()))
        .group_by(m)
        .order(m.desc().nulls_last())
        .load::<(Option<i16>, i64)>(&mut db)
        .await?;
    if groups_in.is_empty() {
        return Err(ViewError::NotFound(Some(context)));
    }
    let mut groups = Vec::with_capacity(groups_in.len());
    for (month, count) in groups_in {
        let month = month.unwrap() as u32; // cant be null when in range!
        let photo = Photo::query(context.is_authorized())
            .filter(p::date.ge(start_of_month(year, month)))
            .filter(p::date.lt(start_of_month(year, month + 1)))
            .order((p::grade.desc().nulls_last(), p::date.asc()))
            .limit(1)
            .first::<Photo>(&mut db)
            .await?;

        groups.push(PhotoLink {
            title: Some(monthname(month).to_string()),
            href: format!("/{year}/{month}/"),
            lable: Some(format!("{count} pictures")),
            id: photo.id,
            size: photo.get_size(SizeTag::Small),
        });
    }

    let pos = Photo::query(context.is_authorized())
        .inner_join(ps::positions)
        .filter(p::date.ge(start_of_year(year)))
        .filter(p::date.lt(start_of_year(year + 1)))
        .select((ps::photo_id, ps::latitude, ps::longitude))
        .load(&mut db)
        .await?
        .into_iter()
        .map(|(p_id, lat, long): (i32, i32, i32)| ((lat, long).into(), p_id))
        .collect::<Vec<_>>();
    Ok(Builder::new().html(|o| {
        templates::index_html(o, &context, &title, &[], &groups, &pos)
    })?)
}

fn start_of_year(year: i32) -> NaiveDateTime {
    start_of_day(year, 1, 1)
}

fn start_of_month(year: i32, month: u32) -> NaiveDateTime {
    if month > 12 {
        start_of_day(year + 1, month - 12, 1)
    } else {
        start_of_day(year, month, 1)
    }
}

fn start_of_day(year: i32, month: u32, day: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(year, month, day)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
}

async fn days_in_month(
    year: i32,
    month: u32,
    context: Context,
) -> Result<Response> {
    let d = day_of_timestamp(p::date);

    let lpath: Vec<Link> = vec![Link::year(year)];
    let title: String = format!("Photos from {} {}", monthname(month), year);
    let mut db = context.db().await?;
    let groups_in = p::photos
        .filter(p::path.not_like("%.CR2"))
        .filter(p::path.not_like("%.dng"))
        .filter(p::is_public.or::<_, Bool>(context.is_authorized()))
        .filter(p::date.ge(start_of_month(year, month)))
        .filter(p::date.lt(start_of_month(year, month + 1)))
        .select((d, count_star()))
        .group_by(d)
        .order(d.desc().nulls_last())
        .load::<(Option<i16>, i64)>(&mut db)
        .await?;
    if groups_in.is_empty() {
        return Err(ViewError::NotFound(Some(context)));
    }
    let mut groups = Vec::with_capacity(groups_in.len());
    for (day, count) in groups_in {
        let day = day.unwrap() as u32;
        let fromdate = start_of_day(year, month, day);
        let photo = Photo::query(context.is_authorized())
            .filter(p::date.ge(fromdate))
            .filter(p::date.lt(fromdate + Duration::days(1)))
            .order((p::grade.desc().nulls_last(), p::date.asc()))
            .limit(1)
            .first::<Photo>(&mut db)
            .await?;

        groups.push(PhotoLink {
            title: Some(format!("{day}")),
            href: format!("/{year}/{month}/{day}"),
            lable: Some(format!("{count} pictures")),
            id: photo.id,
            size: photo.get_size(SizeTag::Small),
        });
    }

    let pos = Photo::query(context.is_authorized())
        .inner_join(ps::positions)
        .filter(p::date.ge(start_of_month(year, month)))
        .filter(p::date.lt(start_of_month(year, month + 1)))
        .select((ps::photo_id, ps::latitude, ps::longitude))
        .load(&mut db)
        .await?
        .into_iter()
        .map(|(p_id, lat, long): (i32, i32, i32)| ((lat, long).into(), p_id))
        .collect::<Vec<_>>();
    Ok(Builder::new().html(|o| {
        templates::index_html(o, &context, &title, &lpath, &groups, &pos)
    })?)
}

async fn all_null_date(context: Context) -> Result<Response> {
    let images = Photo::query(context.is_authorized())
        .filter(p::date.is_null())
        .order(p::path.asc())
        .limit(500)
        .load(&mut context.db().await?)
        .await?
        .iter()
        .map(PhotoLink::no_title)
        .collect::<Vec<_>>();
    Ok(Builder::new().html(|o| {
        templates::index_html(
            o,
            &context,
            "Photos without a date",
            &[],
            &images,
            &[], // Don't care about positions here
        )
    })?)
}

async fn all_for_day(
    year: i32,
    month: u32,
    day: u32,
    range: ImgRange,
    context: Context,
) -> Result<Response> {
    let thedate = start_of_day(year, month, day);

    let photos = Photo::query(context.is_authorized())
        .filter(p::date.ge(thedate))
        .filter(p::date.lt(thedate + Duration::days(1)));
    let (links, coords) =
        links_by_time(&context, photos, range, false).await?;

    Ok(Builder::new().html(|o| {
        templates::index_html(
            o,
            &context,
            &format!("Photos from {} {} {}", day, monthname(month), year),
            &[Link::year(year), Link::month(year, month)],
            &links,
            &coords,
        )
    })?)
}

async fn on_this_day(context: Context) -> Result<Response> {
    let (month, day) = {
        let today = Local::now();
        (today.month(), today.day())
    };
    let mut db = context.db().await?;
    let pos = Photo::query(context.is_authorized())
        .inner_join(ps::positions)
        .filter(filter::month_of_timestamp(p::date).eq(month as i16))
        .filter(filter::day_of_timestamp(p::date).eq(day as i16))
        .select((ps::photo_id, ps::latitude, ps::longitude))
        .load(&mut db)
        .await?
        .into_iter()
        .map(|(p_id, lat, long): (i32, i32, i32)| ((lat, long).into(), p_id))
        .collect::<Vec<_>>();

    let y = year_of_timestamp(p::date);
    let photos_in = p::photos
        .filter(p::path.not_like("%.CR2"))
        .filter(p::path.not_like("%.dng"))
        .filter(p::is_public.or::<_, Bool>(context.is_authorized()))
        .filter(filter::month_of_timestamp(p::date).eq(month as i16))
        .filter(filter::day_of_timestamp(p::date).eq(day as i16))
        .select((y, count_star()))
        .group_by(y)
        .order(y.desc())
        .load::<(Option<i16>, i64)>(&mut db)
        .await?;
    let mut photos = Vec::with_capacity(photos_in.len());
    for (year, count) in photos_in {
        let year = year.unwrap(); // matching date can't be null
        let fromdate = start_of_day(year.into(), month, day);
        let photo = Photo::query(context.is_authorized())
            .filter(p::date.ge(fromdate))
            .filter(p::date.lt(fromdate + Duration::days(1)))
            .order((p::grade.desc().nulls_last(), p::date.asc()))
            .limit(1)
            .first::<Photo>(&mut db)
            .await?;
        photos.push(PhotoLink {
            title: Some(format!("{year}")),
            href: format!("/{year}/{month}/{day}"),
            lable: Some(format!("{count} pictures")),
            id: photo.id,
            size: photo.get_size(SizeTag::Small),
        });
    }
    Ok(Builder::new().html(|o| {
        templates::index_html(
            o,
            &context,
            &format!("Photos from {} {}", day, monthname(month)),
            &[],
            &photos,
            &pos,
        )
    })?)
}

async fn next_image(context: Context, param: FromParam) -> Result<Response> {
    let mut db = context.db().await?;
    let from_date = or_404!(date_of_img(&mut db, param.from).await, context);
    let photo = or_404q!(
        Photo::query(context.is_authorized())
            .select(p::id)
            .filter(
                p::date
                    .gt(from_date)
                    .or(p::date.eq(from_date).and(p::id.gt(param.from))),
            )
            .order((p::date, p::id))
            .first::<i32>(&mut db)
            .await,
        context
    );
    Ok(redirect_to_img(photo))
}

async fn prev_image(context: Context, param: FromParam) -> Result<Response> {
    let mut db = context.db().await?;
    let from_date = or_404!(date_of_img(&mut db, param.from).await, context);
    let photo = or_404q!(
        Photo::query(context.is_authorized())
            .select(p::id)
            .filter(
                p::date
                    .lt(from_date)
                    .or(p::date.eq(from_date).and(p::id.lt(param.from))),
            )
            .order((p::date.desc().nulls_last(), p::id.desc()))
            .first::<i32>(&mut db)
            .await,
        context
    );
    Ok(redirect_to_img(photo))
}

#[derive(Deserialize)]
struct FromParam {
    from: i32,
}

pub async fn date_of_img(
    db: &mut AsyncPgConnection,
    photo_id: i32,
) -> Option<NaiveDateTime> {
    p::photos
        .find(photo_id)
        .select(p::date)
        .first(db)
        .await
        .unwrap_or(None)
}

pub fn monthname(n: u32) -> &'static str {
    match n {
        1 => "january",
        2 => "february",
        3 => "march",
        4 => "april",
        5 => "may",
        6 => "june",
        7 => "july",
        8 => "august",
        9 => "september",
        10 => "october",
        11 => "november",
        12 => "december",
        _ => "non-month",
    }
}
