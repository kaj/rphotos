use super::PhotoLink;
use super::views_by_date::query_date;
use diesel::pg::{Pg, PgConnection};
use diesel::prelude::*;
use models::Photo;
use nickel::Request;
use nickel_diesel::DieselRequestExtensions;
use schema::photos;

pub fn links_by_time<'a>(
    req: &mut Request,
    photos: photos::BoxedQuery<'a, Pg>,
) -> Vec<PhotoLink> {
    let c: &PgConnection = &req.db_conn();
    use schema::photos::dsl::date;
    let photos = if let Some(from_date) = query_date(req, "from") {
        photos.filter(date.ge(from_date))
    } else {
        photos
    };
    let photos = if let Some(to_date) = query_date(req, "to") {
        photos.filter(date.le(to_date))
    } else {
        photos
    };
    let photos = photos.order(date.desc().nulls_last()).load(c).unwrap();
    if let Some(groups) = split_to_groups(&photos) {
        let path = req.path_without_query().unwrap_or("/");
        groups
            .iter()
            .map(|g| PhotoLink::for_group(g, path))
            .collect::<Vec<_>>()
    } else {
        photos.iter().map(PhotoLink::from).collect::<Vec<_>>()
    }
}

pub fn split_to_groups(photos: &[Photo]) -> Option<Vec<&[Photo]>> {
    if photos.len() < 42 {
        return None;
    }
    let wanted_groups = (photos.len() as f64).sqrt() as usize;
    let mut groups = vec![&photos[..]];
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
    let mut largest = 0;
    for (i, g) in groups.iter().enumerate() {
        if g.len() > largest {
            largest = g.len();
            found = i;
        }
    }
    found
}

fn split(group: &[Photo]) -> (&[Photo], &[Photo]) {
    let l = group.len();
    let mut pos = 0;
    let mut dist = 0;
    for i in l / 8..l - l / 8 - 1 {
        let tttt = timestamp(&group[i]) - timestamp(&group[i + 1]);
        if tttt > dist {
            dist = tttt;
            pos = i + 1;
        }
    }
    group.split_at(pos)
}

fn timestamp(p: &Photo) -> i64 {
    p.date.map(|d| d.timestamp()).unwrap_or(0)
}
