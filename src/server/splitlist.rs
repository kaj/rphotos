use super::views_by_date::date_of_img;
use super::{Context, ImgRange, PhotoLink};
use crate::models::{Coord, Photo};
use crate::schema::photos;
use diesel::pg::{Pg, PgConnection};
use diesel::prelude::*;
use log::{debug, info, warn};

pub fn links_by_time<'a>(
    context: &Context,
    photos: photos::BoxedQuery<'a, Pg>,
    range: ImgRange,
    with_date: bool,
) -> (Vec<PhotoLink>, Vec<(Coord, i32)>) {
    let c = context.db().unwrap();
    use crate::schema::photos::dsl::{date, id};
    let photos =
        if let Some(from_date) = range.from.map(|i| date_of_img(&c, i)) {
            photos.filter(date.ge(from_date))
        } else {
            photos
        };
    let photos = if let Some(to_date) = range.to.map(|i| date_of_img(&c, i)) {
        photos.filter(date.le(to_date))
    } else {
        photos
    };
    let photos = photos
        .order((date.desc().nulls_last(), id.desc()))
        .load(&c)
        .unwrap();
    (
        if let Some(groups) = split_to_groups(&photos) {
            let path = context.path_without_query();
            groups
                .iter()
                .map(|g| PhotoLink::for_group(g, path, with_date))
                .collect()
        } else {
            photos
                .iter()
                .map(if with_date {
                    PhotoLink::date_title
                } else {
                    PhotoLink::no_title
                })
                .collect()
        },
        get_positions(&photos, &c),
    )
}

pub fn get_positions(photos: &[Photo], c: &PgConnection) -> Vec<(Coord, i32)> {
    use crate::schema::positions::dsl::*;
    positions
        .filter(photo_id.eq_any(photos.iter().map(|p| p.id)))
        .select((photo_id, latitude, longitude))
        .load(c)
        .map_err(|e| warn!("Failed to load positions: {}", e))
        .unwrap_or_default()
        .into_iter()
        .map(|(p_id, lat, long): (i32, i32, i32)| ((lat, long).into(), p_id))
        .collect()
}

pub fn split_to_groups(photos: &[Photo]) -> Option<Vec<&[Photo]>> {
    let wanted_groups = match photos.len() {
        l if l <= 16 => return None,
        l if l < 81 => 8,
        l if l >= 225 => 15,
        l => (l as f64).sqrt() as usize,
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
        let time = 1 + g.first().map(|p| timestamp(p)).unwrap_or(0)
            - g.last().map(|p| timestamp(p)).unwrap_or(0);
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
    info!("Splitting a group len {} at {}", l, pos);
    group.split_at(pos)
}

fn timestamp(p: &Photo) -> i64 {
    p.date.map(|d| d.timestamp()).unwrap_or(0)
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
