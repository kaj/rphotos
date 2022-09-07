use super::splitlist::links_by_time;
use super::{
    redirect_to_img, wrap, Context, ContextFilter, ImgRange, Link, PhotoLink,
    Result, ViewError,
};
use crate::models::{Photo, SizeTag};
use crate::templates::{self, RenderRucte};
use chrono::naive::{NaiveDate, NaiveDateTime};
use chrono::{Datelike, Duration, Local};
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Integer, Nullable};
use serde::Deserialize;
use warp::filters::BoxedFilter;
use warp::http::response::Builder;
use warp::path::{end, param};
use warp::query::query;
use warp::reply::Response;
use warp::{get, path, Filter};

pub fn routes(s: ContextFilter) -> BoxedFilter<(Response,)> {
    let s = move || s.clone();
    let root = end().and(get()).and(s()).map(all_years);
    let nodate = path("0").and(end()).and(get()).and(s()).map(all_null_date);
    let year = param().and(end()).and(get()).and(s()).map(months_in_year);
    let month = param()
        .and(param())
        .and(end())
        .and(get())
        .and(s())
        .map(days_in_month);
    let day = param()
        .and(param())
        .and(param())
        .and(end())
        .and(query())
        .and(get())
        .and(s())
        .map(all_for_day);

    let this = path("thisday")
        .and(end())
        .and(get())
        .and(s())
        .map(on_this_day);
    let next = path("next")
        .and(end())
        .and(get())
        .and(s())
        .and(query())
        .map(next_image);
    let prev = path("prev")
        .and(end())
        .and(get())
        .and(s())
        .and(query())
        .map(prev_image);

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

fn all_years(context: Context) -> Result<Response> {
    use crate::schema::photos::dsl as p;
    let db = context.db()?;
    let groups = Photo::query(context.is_authorized())
        .select(sql::<(Nullable<Integer>, BigInt)>(
            "cast(extract(year from date) as int) y, count(*)",
        ))
        .group_by(sql::<Nullable<Integer>>("y"))
        .order(sql::<Nullable<Integer>>("y").desc().nulls_last())
        .load::<(Option<i32>, i64)>(&db)?
        .iter()
        .map(|&(year, count)| {
            let q = Photo::query(context.is_authorized())
                .order((p::grade.desc().nulls_last(), p::date.asc()))
                .limit(1);
            let photo = if let Some(year) = year {
                q.filter(p::date.ge(start_of_year(year)))
                    .filter(p::date.lt(start_of_year(year + 1)))
            } else {
                q.filter(p::date.is_null())
            };
            let photo = photo.first::<Photo>(&db)?;
            Ok(PhotoLink {
                title: Some(
                    year.map(|y| format!("{}", y))
                        .unwrap_or_else(|| "-".to_string()),
                ),
                href: format!("/{}/", year.unwrap_or(0)),
                lable: Some(format!("{} images", count)),
                id: photo.id,
                size: photo.get_size(SizeTag::Small),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Builder::new().html(|o| {
        templates::index(o, &context, "All photos", &[], &groups, &[])
    })?)
}

fn start_of_year(year: i32) -> NaiveDateTime {
    NaiveDate::from_ymd(year, 1, 1).and_hms(0, 0, 0)
}

fn months_in_year(year: i32, context: Context) -> Result<Response> {
    use crate::schema::photos::dsl as p;

    let title: String = format!("Photos from {}", year);
    let db = context.db()?;
    let groups = Photo::query(context.is_authorized())
        .filter(p::date.ge(start_of_year(year)))
        .filter(p::date.lt(start_of_year(year + 1)))
        .select(sql::<(Integer, BigInt)>(
            "cast(extract(month from date) as int) m, count(*)",
        ))
        .group_by(sql::<Integer>("m"))
        .order(sql::<Integer>("m").desc().nulls_last())
        .load::<(i32, i64)>(&db)?
        .iter()
        .map(|&(month, count)| {
            let month = month as u32;
            let photo = Photo::query(context.is_authorized())
                .filter(p::date.ge(start_of_month(year, month)))
                .filter(p::date.lt(start_of_month(year, month + 1)))
                .order((p::grade.desc().nulls_last(), p::date.asc()))
                .limit(1)
                .first::<Photo>(&db)?;

            Ok(PhotoLink {
                title: Some(monthname(month).to_string()),
                href: format!("/{}/{}/", year, month),
                lable: Some(format!("{} pictures", count)),
                id: photo.id,
                size: photo.get_size(SizeTag::Small),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    if groups.is_empty() {
        Err(ViewError::NotFound(Some(context)))
    } else {
        use crate::schema::positions::dsl::{
            latitude, longitude, photo_id, positions,
        };
        let pos = Photo::query(context.is_authorized())
            .inner_join(positions)
            .filter(p::date.ge(start_of_year(year)))
            .filter(p::date.lt(start_of_year(year + 1)))
            .select((photo_id, latitude, longitude))
            .load(&db)?
            .into_iter()
            .map(|(p_id, lat, long): (i32, i32, i32)| {
                ((lat, long).into(), p_id)
            })
            .collect::<Vec<_>>();
        Ok(Builder::new().html(|o| {
            templates::index(o, &context, &title, &[], &groups, &pos)
        })?)
    }
}

fn start_of_month(year: i32, month: u32) -> NaiveDateTime {
    let date = if month > 12 {
        NaiveDate::from_ymd(year + 1, month - 12, 1)
    } else {
        NaiveDate::from_ymd(year, month, 1)
    };
    date.and_hms(0, 0, 0)
}

fn days_in_month(year: i32, month: u32, context: Context) -> Result<Response> {
    use crate::schema::photos::dsl as p;

    let lpath: Vec<Link> = vec![Link::year(year)];
    let title: String = format!("Photos from {} {}", monthname(month), year);
    let db = context.db()?;
    let groups = Photo::query(context.is_authorized())
        .filter(p::date.ge(start_of_month(year, month)))
        .filter(p::date.lt(start_of_month(year, month + 1)))
        .select(sql::<(Integer, BigInt)>(
            "cast(extract(day from date) as int) d, count(*)",
        ))
        .group_by(sql::<Integer>("d"))
        .order(sql::<Integer>("d").desc().nulls_last())
        .load::<(i32, i64)>(&db)?
        .iter()
        .map(|&(day, count)| {
            let day = day as u32;
            let fromdate =
                NaiveDate::from_ymd(year, month, day).and_hms(0, 0, 0);
            let photo = Photo::query(context.is_authorized())
                .filter(p::date.ge(fromdate))
                .filter(p::date.lt(fromdate + Duration::days(1)))
                .order((p::grade.desc().nulls_last(), p::date.asc()))
                .limit(1)
                .first::<Photo>(&db)?;

            Ok(PhotoLink {
                title: Some(format!("{}", day)),
                href: format!("/{}/{}/{}", year, month, day),
                lable: Some(format!("{} pictures", count)),
                id: photo.id,
                size: photo.get_size(SizeTag::Small),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    if groups.is_empty() {
        Err(ViewError::NotFound(Some(context)))
    } else {
        use crate::schema::positions::dsl as ps;
        let pos = Photo::query(context.is_authorized())
            .inner_join(ps::positions)
            .filter(p::date.ge(start_of_month(year, month)))
            .filter(p::date.lt(start_of_month(year, month + 1)))
            .select((ps::photo_id, ps::latitude, ps::longitude))
            .load(&db)?
            .into_iter()
            .map(|(p_id, lat, long): (i32, i32, i32)| {
                ((lat, long).into(), p_id)
            })
            .collect::<Vec<_>>();
        Ok(Builder::new().html(|o| {
            templates::index(o, &context, &title, &lpath, &groups, &pos)
        })?)
    }
}

fn all_null_date(context: Context) -> Result<Response> {
    use crate::schema::photos::dsl as p;
    let images = Photo::query(context.is_authorized())
        .filter(p::date.is_null())
        .order(p::path.asc())
        .limit(500)
        .load(&context.db()?)?
        .iter()
        .map(PhotoLink::no_title)
        .collect::<Vec<_>>();
    Ok(Builder::new().html(|o| {
        templates::index(
            o,
            &context,
            "Photos without a date",
            &[],
            &images,
            &[], // Don't care about positions here
        )
    })?)
}

fn all_for_day(
    year: i32,
    month: u32,
    day: u32,
    range: ImgRange,
    context: Context,
) -> Result<Response> {
    let thedate = NaiveDate::from_ymd(year, month, day).and_hms(0, 0, 0);
    use crate::schema::photos::dsl as p;

    let photos = Photo::query(context.is_authorized())
        .filter(p::date.ge(thedate))
        .filter(p::date.lt(thedate + Duration::days(1)));
    let (links, coords) = links_by_time(&context, photos, range, false)?;

    if links.is_empty() {
        Err(ViewError::NotFound(Some(context)))
    } else {
        Ok(Builder::new().html(|o| {
            templates::index(
                o,
                &context,
                &format!("Photos from {} {} {}", day, monthname(month), year),
                &[Link::year(year), Link::month(year, month)],
                &links,
                &coords,
            )
        })?)
    }
}

fn on_this_day(context: Context) -> Result<Response> {
    use crate::schema::photos::dsl as p;
    use crate::schema::positions::dsl as ps;

    let (month, day) = {
        let today = Local::now();
        (today.month(), today.day())
    };
    let db = context.db()?;
    let pos = Photo::query(context.is_authorized())
        .inner_join(ps::positions)
        .filter(
            sql("extract(month from date)=").bind::<Integer, _>(month as i32),
        )
        .filter(sql("extract(day from date)=").bind::<Integer, _>(day as i32))
        .select((ps::photo_id, ps::latitude, ps::longitude))
        .load(&db)?
        .into_iter()
        .map(|(p_id, lat, long): (i32, i32, i32)| ((lat, long).into(), p_id))
        .collect::<Vec<_>>();

    let photos = Photo::query(context.is_authorized())
        .select(sql::<(Integer, BigInt)>(
            "cast(extract(year from date) as int) y, count(*)",
        ))
        .group_by(sql::<Integer>("y"))
        .filter(
            sql("extract(month from date)=").bind::<Integer, _>(month as i32),
        )
        .filter(sql("extract(day from date)=").bind::<Integer, _>(day as i32))
        .order(sql::<Integer>("y").desc())
        .load::<(i32, i64)>(&db)?
        .iter()
        .map(|&(year, count)| {
            let fromdate =
                NaiveDate::from_ymd(year, month as u32, day).and_hms(0, 0, 0);
            let photo = Photo::query(context.is_authorized())
                .filter(p::date.ge(fromdate))
                .filter(p::date.lt(fromdate + Duration::days(1)))
                .order((p::grade.desc().nulls_last(), p::date.asc()))
                .limit(1)
                .first::<Photo>(&db)?;
            Ok(PhotoLink {
                title: Some(format!("{}", year)),
                href: format!("/{}/{}/{}", year, month, day),
                lable: Some(format!("{} pictures", count)),
                id: photo.id,
                size: photo.get_size(SizeTag::Small),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Builder::new().html(|o| {
        templates::index(
            o,
            &context,
            &format!("Photos from {} {}", day, monthname(month)),
            &[],
            &photos,
            &pos,
        )
    })?)
}

fn next_image(context: Context, param: FromParam) -> Result<Response> {
    use crate::schema::photos::dsl as p;
    let db = context.db()?;
    let from_date = or_404!(date_of_img(&db, param.from), context);
    let photo = or_404q!(
        Photo::query(context.is_authorized())
            .select(p::id)
            .filter(
                p::date
                    .gt(from_date)
                    .or(p::date.eq(from_date).and(p::id.gt(param.from))),
            )
            .order((p::date, p::id))
            .first::<i32>(&db),
        context
    );
    Ok(redirect_to_img(photo))
}

fn prev_image(context: Context, param: FromParam) -> Result<Response> {
    use crate::schema::photos::dsl as p;
    let db = context.db()?;
    let from_date = or_404!(date_of_img(&db, param.from), context);
    let photo = or_404q!(
        Photo::query(context.is_authorized())
            .select(p::id)
            .filter(
                p::date
                    .lt(from_date)
                    .or(p::date.eq(from_date).and(p::id.lt(param.from))),
            )
            .order((p::date.desc().nulls_last(), p::id.desc()))
            .first::<i32>(&db),
        context
    );
    Ok(redirect_to_img(photo))
}

#[derive(Deserialize)]
struct FromParam {
    from: i32,
}

pub fn date_of_img(db: &PgConnection, photo_id: i32) -> Option<NaiveDateTime> {
    use crate::schema::photos::dsl as p;
    p::photos
        .find(photo_id)
        .select(p::date)
        .first(db)
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
