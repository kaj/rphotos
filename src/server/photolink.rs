use crate::models::{Photo, SizeTag};
use chrono::Datelike;

pub struct PhotoLink {
    pub title: Option<String>,
    pub href: String,
    pub id: i32,
    pub size: (u32, u32),
    pub lable: Option<String>,
}

impl PhotoLink {
    pub fn for_group(
        g: &[Photo],
        base_url: &str,
        with_date: bool,
    ) -> PhotoLink {
        if g.len() == 1 {
            if with_date {
                PhotoLink::date_title(&g[0])
            } else {
                PhotoLink::no_title(&g[0])
            }
        } else {
            fn imgscore(p: &Photo) -> i16 {
                // Only score below 19 is worse than ungraded.
                p.grade.unwrap_or(19) * if p.is_public { 5 } else { 4 }
            }
            let photo = g.iter().max_by_key(|p| imgscore(p)).unwrap();
            let (title, lable) = {
                let from = g.last().and_then(|p| p.date);
                let to = g.first().and_then(|p| p.date);
                if let (Some(from), Some(to)) = (from, to) {
                    if from.date() == to.date() {
                        (
                            Some(from.format("%F").to_string()),
                            format!(
                                "{} - {} ({})",
                                from.format("%R"),
                                to.format("%R"),
                                g.len(),
                            ),
                        )
                    } else if from.year() == to.year() {
                        if from.month() == to.month() {
                            (
                                Some(from.format("%Y-%m").to_string()),
                                format!(
                                    "{} - {} ({})",
                                    from.format("%F"),
                                    to.format("%d"),
                                    g.len(),
                                ),
                            )
                        } else {
                            (
                                Some(from.format("%Y").to_string()),
                                format!(
                                    "{} - {} ({})",
                                    from.format("%F"),
                                    to.format("%m-%d"),
                                    g.len(),
                                ),
                            )
                        }
                    } else {
                        (
                            None,
                            format!(
                                "{} - {} ({})",
                                from.format("%F"),
                                to.format("%F"),
                                g.len(),
                            ),
                        )
                    }
                } else {
                    (
                        None,
                        format!(
                            "{} - {} ({})",
                            from.map(|d| format!("{}", d.format("%F %R")))
                                .unwrap_or_else(|| "-".to_string()),
                            to.map(|d| format!("{}", d.format("%F %R")))
                                .unwrap_or_else(|| "-".to_string()),
                            g.len(),
                        ),
                    )
                }
            };
            let title = if with_date { title } else { None };
            PhotoLink {
                title,
                href: format!(
                    "{}?from={}&to={}",
                    base_url,
                    g.last().map(|p| p.id).unwrap_or(0),
                    g.first().map(|p| p.id).unwrap_or(0),
                ),
                id: photo.id,
                size: photo.get_size(SizeTag::Small),
                lable: Some(lable),
            }
        }
    }
    pub fn date_title(p: &Photo) -> PhotoLink {
        PhotoLink {
            title: p.date.map(|d| d.format("%F").to_string()),
            href: format!("/img/{}", p.id),
            id: p.id,
            size: p.get_size(SizeTag::Small),
            lable: p.date.map(|d| d.format("%T").to_string()),
        }
    }
    pub fn no_title(p: &Photo) -> PhotoLink {
        PhotoLink {
            title: None, // p.date.map(|d| d.format("%F").to_string()),
            href: format!("/img/{}", p.id),
            id: p.id,
            size: p.get_size(SizeTag::Small),
            lable: p.date.map(|d| d.format("%T").to_string()),
        }
    }
    pub fn is_portrait(&self) -> bool {
        self.size.1 > self.size.0
    }
}
