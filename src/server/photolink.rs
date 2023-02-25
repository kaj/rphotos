use super::urlstring::UrlString;
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
        url: UrlString,
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
                        let (from_year, to_year) = (from.year(), to.year());
                        let to_year = if from_year / 100 == to_year / 100 {
                            to_year % 100
                        } else {
                            to_year
                        };
                        (
                            Some(format!("{from_year} - {to_year}")),
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
                            from.map_or_else(
                                || "-".to_string(),
                                |d| format!("{}", d.format("%F %R"))
                            ),
                            to.map_or_else(
                                || "-".to_string(),
                                |d| format!("{}", d.format("%F %R"))
                            ),
                            g.len(),
                        ),
                    )
                }
            };
            let title = if with_date { title } else { None };
            let mut url = url;
            if let Some(last) = g.last() {
                url.query("from", last.id);
            }
            if let Some(first) = g.first() {
                url.query("to", first.id);
            }
            PhotoLink {
                title,
                href: url.into(),
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
