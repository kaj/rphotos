use models::Photo;

pub fn split_to_groups(photos: &[Photo]) -> Vec<&[Photo]> {
    let wanted_groups = (photos.len() as f64).sqrt() as usize;
    let mut groups = vec![&photos[..]];
    while groups.len() < wanted_groups {
        let i = find_largest(&groups);
        let (a, b) = split(groups[i]);
        groups[i] = a;
        groups.insert(i + 1, b);
    }
    groups
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
