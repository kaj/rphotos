use super::splitlist::links_by_time;
use super::{Link, PhotoLink, SizeTag};
use chrono::naive::{NaiveDate, NaiveDateTime};
use chrono::Duration as ChDuration;
use diesel::dsl::sql;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Integer, Nullable};
use models::Photo;
use nickel::extensions::response::Redirect;
use nickel::{MiddlewareResult, QueryString, Request, Response};
use nickel_diesel::DieselRequestExtensions;
use nickel_jwt_session::SessionRequestExtensions;
use server::nickelext::MyResponse;
use templates;
use time;

pub fn all_years<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    use schema::photos::dsl::{date, grade};
    let c: &PgConnection = &req.db_conn();

    let groups = Photo::query(req.authorized_user().is_some())
        .select(sql::<(Nullable<Integer>, BigInt)>(
            "cast(extract(year from date) as int) y, count(*)",
        ))
        .group_by(sql::<Nullable<Integer>>("y"))
        .order(sql::<Nullable<Integer>>("y").desc().nulls_last())
        .load::<(Option<i32>, i64)>(c)
        .unwrap()
        .iter()
        .map(|&(year, count)| {
            let q = Photo::query(req.authorized_user().is_some())
                .order((grade.desc().nulls_last(), date.asc()))
                .limit(1);
            let photo = if let Some(year) = year {
                q.filter(date.ge(start_of_year(year)))
                    .filter(date.lt(start_of_year(year + 1)))
            } else {
                q.filter(date.is_null())
            };
            let photo = photo.first::<Photo>(c).unwrap();
            PhotoLink {
                title: Some(
                    year.map(|y| format!("{}", y))
                        .unwrap_or_else(|| "-".to_string()),
                ),
                href: format!("/{}/", year.unwrap_or(0)),
                lable: Some(format!("{} images", count)),
                id: photo.id,
                size: photo.get_size(SizeTag::Small.px()),
            }
        })
        .collect::<Vec<_>>();

    res.ok(|o| templates::index(o, req, "All photos", &[], &groups, &[]))
}

fn start_of_year(year: i32) -> NaiveDateTime {
    NaiveDate::from_ymd(year, 1, 1).and_hms(0, 0, 0)
}

pub fn months_in_year<'mw>(
    req: &mut Request,
    res: Response<'mw>,
    year: i32,
) -> MiddlewareResult<'mw> {
    use schema::photos::dsl::{date, grade};
    let c: &PgConnection = &req.db_conn();

    let title: String = format!("Photos from {}", year);
    let groups = Photo::query(req.authorized_user().is_some())
        .filter(date.ge(start_of_year(year)))
        .filter(date.lt(start_of_year(year + 1)))
        .select(sql::<(Integer, BigInt)>(
            "cast(extract(month from date) as int) m, count(*)",
        ))
        .group_by(sql::<Integer>("m"))
        .order(sql::<Integer>("m").desc().nulls_last())
        .load::<(i32, i64)>(c)
        .unwrap()
        .iter()
        .map(|&(month, count)| {
            let month = month as u32;
            let photo = Photo::query(req.authorized_user().is_some())
                .filter(date.ge(start_of_month(year, month)))
                .filter(date.lt(start_of_month(year, month + 1)))
                .order((grade.desc().nulls_last(), date.asc()))
                .limit(1)
                .first::<Photo>(c)
                .unwrap();

            PhotoLink {
                title: Some(monthname(month).to_string()),
                href: format!("/{}/{}/", year, month),
                lable: Some(format!("{} pictures", count)),
                id: photo.id,
                size: photo.get_size(SizeTag::Small.px()),
            }
        })
        .collect::<Vec<_>>();

    if groups.is_empty() {
        res.not_found("No such image")
    } else {
        use schema::positions::dsl::{
            latitude, longitude, photo_id, positions,
        };
        let pos = Photo::query(req.authorized_user().is_some())
            .inner_join(positions)
            .filter(date.ge(start_of_year(year)))
            .filter(date.lt(start_of_year(year + 1)))
            .select((photo_id, latitude, longitude))
            .load(c)
            .map_err(|e| warn!("Failed to load positions: {}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|(p_id, lat, long): (i32, i32, i32)| {
                ((lat, long).into(), p_id)
            })
            .collect::<Vec<_>>();
        res.ok(|o| templates::index(o, req, &title, &[], &groups, &pos))
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

pub fn days_in_month<'mw>(
    req: &mut Request,
    res: Response<'mw>,
    year: i32,
    month: u32,
) -> MiddlewareResult<'mw> {
    use schema::photos::dsl::{date, grade};
    let c: &PgConnection = &req.db_conn();

    let lpath: Vec<Link> = vec![Link::year(year)];
    let title: String = format!("Photos from {} {}", monthname(month), year);
    let groups = Photo::query(req.authorized_user().is_some())
        .filter(date.ge(start_of_month(year, month)))
        .filter(date.lt(start_of_month(year, month + 1)))
        .select(sql::<(Integer, BigInt)>(
            "cast(extract(day from date) as int) d, count(*)",
        ))
        .group_by(sql::<Integer>("d"))
        .order(sql::<Integer>("d").desc().nulls_last())
        .load::<(i32, i64)>(c)
        .unwrap()
        .iter()
        .map(|&(day, count)| {
            let day = day as u32;
            let fromdate =
                NaiveDate::from_ymd(year, month, day).and_hms(0, 0, 0);
            let photo = Photo::query(req.authorized_user().is_some())
                .filter(date.ge(fromdate))
                .filter(date.lt(fromdate + ChDuration::days(1)))
                .order((grade.desc().nulls_last(), date.asc()))
                .limit(1)
                .first::<Photo>(c)
                .unwrap();

            PhotoLink {
                title: Some(format!("{}", day)),
                href: format!("/{}/{}/{}", year, month, day),
                lable: Some(format!("{} pictures", count)),
                id: photo.id,
                size: photo.get_size(SizeTag::Small.px()),
            }
        })
        .collect::<Vec<_>>();

    if groups.is_empty() {
        res.not_found("No such image")
    } else {
        use schema::positions::dsl::{
            latitude, longitude, photo_id, positions,
        };
        let pos = Photo::query(req.authorized_user().is_some())
            .inner_join(positions)
            .filter(date.ge(start_of_month(year, month)))
            .filter(date.lt(start_of_month(year, month + 1)))
            .select((photo_id, latitude, longitude))
            .load(c)
            .map_err(|e| warn!("Failed to load positions: {}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|(p_id, lat, long): (i32, i32, i32)| {
                ((lat, long).into(), p_id)
            })
            .collect::<Vec<_>>();
        res.ok(|o| templates::index(o, req, &title, &lpath, &groups, &pos))
    }
}

pub fn all_null_date<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    use schema::photos::dsl::{date, path};

    let c: &PgConnection = &req.db_conn();
    res.ok(|o| {
        templates::index(
            o,
            req,
            "Photos without a date",
            &[],
            &Photo::query(req.authorized_user().is_some())
                .filter(date.is_null())
                .order(path.asc())
                .limit(500)
                .load(c)
                .unwrap()
                .iter()
                .map(PhotoLink::from)
                .collect::<Vec<_>>(),
            &[], // Don't care about positions here
        )
    })
}

pub fn all_for_day<'mw>(
    req: &mut Request,
    res: Response<'mw>,
    year: i32,
    month: u32,
    day: u32,
) -> MiddlewareResult<'mw> {
    let thedate = NaiveDate::from_ymd(year, month, day).and_hms(0, 0, 0);
    use schema::photos::dsl::date;

    let photos = Photo::query(req.authorized_user().is_some())
        .filter(date.ge(thedate))
        .filter(date.lt(thedate + ChDuration::days(1)));
    let (links, coords) = links_by_time(req, photos);

    if links.is_empty() {
        res.not_found("No such image")
    } else {
        res.ok(|o| {
            templates::index(
                o,
                req,
                &format!("Photos from {} {} {}", day, monthname(month), year),
                &[Link::year(year), Link::month(year, month)],
                &links,
                &coords,
            )
        })
    }
}

pub fn on_this_day<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    use schema::photos::dsl::{date, grade};
    use schema::positions::dsl::{latitude, longitude, photo_id, positions};
    let c: &PgConnection = &req.db_conn();

    let (month, day) = {
        let now = time::now();
        (now.tm_mon as u32 + 1, now.tm_mday as u32)
    };
    let pos = Photo::query(req.authorized_user().is_some())
        .inner_join(positions)
        .filter(
            sql("extract(month from date)=").bind::<Integer, _>(month as i32),
        )
        .filter(sql("extract(day from date)=").bind::<Integer, _>(day as i32))
        .select((photo_id, latitude, longitude))
        .load(c)
        .map_err(|e| warn!("Failed to load positions: {}", e))
        .unwrap_or_default()
        .into_iter()
        .map(|(p_id, lat, long): (i32, i32, i32)| ((lat, long).into(), p_id))
        .collect::<Vec<_>>();

    res.ok(|o| {
        templates::index(
            o,
            req,
            &format!("Photos from {} {}", day, monthname(month)),
            &[],
            &Photo::query(req.authorized_user().is_some())
                .select(sql::<(Integer, BigInt)>(
                    "cast(extract(year from date) as int) y, count(*)",
                ))
                .group_by(sql::<Integer>("y"))
                .filter(
                    sql("extract(month from date)=")
                        .bind::<Integer, _>(month as i32),
                )
                .filter(
                    sql("extract(day from date)=")
                        .bind::<Integer, _>(day as i32),
                )
                .order(sql::<Integer>("y").desc())
                .load::<(i32, i64)>(c)
                .unwrap()
                .iter()
                .map(|&(year, count)| {
                    let fromdate =
                        NaiveDate::from_ymd(year, month as u32, day)
                            .and_hms(0, 0, 0);
                    let photo = Photo::query(req.authorized_user().is_some())
                        .filter(date.ge(fromdate))
                        .filter(date.lt(fromdate + ChDuration::days(1)))
                        .order((grade.desc().nulls_last(), date.asc()))
                        .limit(1)
                        .first::<Photo>(c)
                        .unwrap();

                    PhotoLink {
                        title: Some(format!("{}", year)),
                        href: format!("/{}/{}/{}", year, month, day),
                        lable: Some(format!("{} pictures", count)),
                        id: photo.id,
                        size: photo.get_size(SizeTag::Small.px()),
                    }
                })
                .collect::<Vec<_>>(),
            &pos,
        )
    })
}

pub fn next_image<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    use schema::photos::dsl::{date, id};
    if let Some((from_id, from_date)) = query_date(req, "from") {
        let q = Photo::query(req.authorized_user().is_some())
            .select(id)
            .filter(
                date.gt(from_date)
                    .or(date.eq(from_date).and(id.gt(from_id))),
            )
            .order((date, id));
        let c: &PgConnection = &req.db_conn();
        if let Ok(photo) = q.first::<i32>(c) {
            return res.redirect(format!("/img/{}", photo)); // to photo_details
        }
    }
    res.not_found("No such image")
}

pub fn prev_image<'mw>(
    req: &mut Request,
    res: Response<'mw>,
) -> MiddlewareResult<'mw> {
    use schema::photos::dsl::{date, id};
    if let Some((from_id, from_date)) = query_date(req, "from") {
        let q = Photo::query(req.authorized_user().is_some())
            .select(id)
            .filter(
                date.lt(from_date)
                    .or(date.eq(from_date).and(id.lt(from_id))),
            )
            .order((date.desc().nulls_last(), id.desc()));
        let c: &PgConnection = &req.db_conn();
        if let Ok(photo) = q.first::<i32>(c) {
            return res.redirect(format!("/img/{}", photo)); // to photo_details
        }
    }
    res.not_found("No such image")
}

pub fn query_date(
    req: &mut Request,
    name: &str,
) -> Option<(i32, NaiveDateTime)> {
    req.query()
        .get(name)
        .and_then(|s| s.parse().ok())
        .and_then(|i: i32| {
            use schema::photos::dsl::{date, photos};
            let c: &PgConnection = &req.db_conn();
            photos
                .find(i)
                .select(date)
                .first(c)
                .unwrap_or(None)
                .map(|d| (i, d))
        })
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
