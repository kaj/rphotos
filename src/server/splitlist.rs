use super::urlstring::UrlString;
use super::views_by_date::date_of_img;
use super::{Context, ImgRange, PhotoLink, Result, SomeVec, ViewError};
use crate::models::{Coord, Photo};
use crate::schema::photos;
use crate::schema::photos::dsl as p;
use crate::schema::positions::dsl as ps;
use chrono::NaiveDateTime;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use tracing::debug;

pub async fn links_by_time(
    context: &Context,
    photos: photos::BoxedQuery<'_, Pg>,
    range: ImgRange,
    with_date: bool,
) -> Result<(Vec<PhotoLink>, Vec<(Coord, i32)>)> {
    let mut c = context.db().await?;
    let photos = if let Some(fr) = date_of_opt_img(&mut c, range.from).await {
        photos.filter(p::date.ge(fr))
    } else {
        photos
    };
    let photos = if let Some(to) = date_of_opt_img(&mut c, range.to).await {
        photos.filter(p::date.le(to))
    } else {
        photos
    };
    let photos = photos
        .order((p::date.desc().nulls_last(), p::id.desc()))
        .left_join(ps::positions)
        .select((
            Photo::as_select(),
            ((ps::latitude, ps::longitude), ps::photo_id).nullable(),
        ))
        .load::<(Photo, Option<(Coord, i32)>)>(&mut c)
        .await?;

    if photos.is_empty() {
        return Err(ViewError::NotFound(None));
    }

    let (photos, positions): (Vec<_>, SomeVec<_>) = photos.into_iter().unzip();
    let baseurl = UrlString::new(context.path_without_query());
    Ok((
        split_to_group_links(&photos, &baseurl, with_date),
        positions.0,
    ))
}

async fn date_of_opt_img(
    db: &mut AsyncPgConnection,
    img: Option<i32>,
) -> Option<NaiveDateTime> {
    date_of_img(db, img?).await
}

pub fn split_to_group_links(
    photos: &[Photo],
    path: &UrlString,
    with_date: bool,
) -> Vec<PhotoLink> {
    if let Some(groups) = split_to_groups(photos) {
        groups
            .iter()
            .map(|g| PhotoLink::for_group(g, path.clone(), with_date))
            .collect()
    } else {
        let make_link = if with_date {
            PhotoLink::date_title
        } else {
            PhotoLink::no_title
        };
        photos.iter().map(make_link).collect()
    }
}

fn split_to_groups(photos: &[Photo]) -> Option<Vec<&[Photo]>> {
    let wanted_groups = match photos.len() {
        l if l <= 18 => return None,
        l if l < 120 => 10,
        l if l < 256 => (l as f64).sqrt() as usize,
        _ => 16,
    };
    let mut groups = vec![photos];
    while groups.len() < wanted_groups {
        let i = find_largest(&groups);
        let (a, b) = split(groups[i]);
        groups[i] = a;
        groups.insert(i + 1, b);
    }
    Some(groups)
}

fn find_largest(groups: &[&[Photo]]) -> usize {
    let mut found = 0;
    let mut largest = 0.0;
    for (i, g) in groups.iter().enumerate() {
        let time = 1 + g.iter().next().map_or(0, timestamp)
            - g.last().map_or(0, timestamp);
        let score = (g.len() as f64).powi(3) * (time as f64);
        if score > largest {
            largest = score;
            found = i;
        }
    }
    found
}

fn split(group: &[Photo]) -> (&[Photo], &[Photo]) {
    fn gradeval(p: &Photo) -> u64 {
        1 + p.grade.unwrap_or(30) as u64
    }
    let l = group.len();
    let gradesum = group.iter().fold(0u64, |sum, p| sum + gradeval(p));
    let mut lsum = 0;
    let edge = l / 16;
    let mut pos = 0;
    let mut largest = 0;
    for i in edge..l - 1 - edge {
        let interval = timestamp(&group[i]) - timestamp(&group[i + 1]);
        let interval = if interval < 0 {
            panic!(
                "Got images {:?}, {:?} in wrong order",
                group[i],
                group[i + 1]
            )
        } else {
            interval as u64
        };
        lsum += gradeval(&group[i]);
        let rsum = gradesum - lsum;
        let score = (interval + 1) * (lsum * rsum);
        debug!("Pos #{} score: {}", i, score);
        if score > largest {
            largest = score;
            pos = i + 1;
        }
    }
    debug!("Splitting a group len {} at {}", l, pos);
    group.split_at(pos)
}

fn timestamp(p: &Photo) -> i64 {
    p.date.map_or(0, |d| d.timestamp())
}

#[test]
fn split_two() {
    let photos = [
        Photo::mock(2018, 08, 31, 21, 45, 48),
        Photo::mock(2018, 08, 31, 21, 45, 12),
    ];
    assert_eq!(paths(split(&photos)), paths((&photos[..1], &photos[1..])));
}

#[test]
fn split_group_by_time() {
    let photos = [
        Photo::mock(2018, 08, 31, 21, 45, 22),
        Photo::mock(2018, 08, 31, 21, 45, 20),
        Photo::mock(2018, 08, 31, 21, 45, 18),
        Photo::mock(2018, 08, 31, 21, 45, 16),
        Photo::mock(2018, 08, 31, 21, 45, 14),
        Photo::mock(2018, 08, 31, 21, 45, 12),
        Photo::mock(2018, 08, 31, 21, 45, 10),
        Photo::mock(2018, 08, 15, 13, 15, 0),
        Photo::mock(2018, 08, 15, 13, 14, 0),
    ];
    assert_eq!(paths(split(&photos)), paths((&photos[..7], &photos[7..])));
}

#[test]
fn split_group_same_time() {
    let photos = [
        Photo::mock(2018, 08, 31, 21, 45, 22),
        Photo::mock(2018, 08, 31, 21, 45, 22),
        Photo::mock(2018, 08, 31, 21, 45, 22),
        Photo::mock(2018, 08, 31, 21, 45, 22),
    ];
    assert_eq!(paths(split(&photos)), paths((&photos[..2], &photos[2..])));
}

#[cfg(test)]
fn paths<'a>(
    (a, b): (&'a [Photo], &'a [Photo]),
) -> (Vec<&'a str>, Vec<&'a str>) {
    (
        a.iter().map(|p| p.path.as_ref()).collect(),
        b.iter().map(|p| p.path.as_ref()).collect(),
    )
}
