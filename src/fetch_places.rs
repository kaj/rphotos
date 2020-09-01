use crate::dbopt::PgPool;
use crate::models::{Coord, Place};
use crate::DbOpt;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use log::{debug, info, warn};
use reqwest::{self, Client, Response};
use serde_json::Value;
use slug::slugify;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct Fetchplaces {
    #[structopt(flatten)]
    db: DbOpt,
    #[structopt(flatten)]
    overpass: OverpassOpt,

    /// Max number of photos to use for --auto
    #[structopt(long, short, default_value = "5")]
    limit: i64,
    /// Fetch data for photos with position but lacking places.
    #[structopt(long, short)]
    auto: bool,
    /// Image ids to fetch place data for
    photos: Vec<i32>,
}

impl Fetchplaces {
    pub async fn run(&self) -> Result<(), super::adm::result::Error> {
        let db = self.db.create_pool()?;
        if self.auto {
            println!("Should find {} photos to fetch places for", self.limit);
            use crate::schema::photo_places::dsl as place;
            use crate::schema::positions::dsl as pos;
            let result = pos::positions
                .select((pos::photo_id, (pos::latitude, pos::longitude)))
                .filter(pos::photo_id.ne_all(
                    place::photo_places.select(place::photo_id).distinct(),
                ))
                .order(pos::photo_id.desc())
                .limit(self.limit)
                .load::<(i32, Coord)>(&db.get()?)?;
            for (photo_id, coord) in result {
                println!("Find places for #{}, {:?}", photo_id, coord);
                self.overpass.update_image_places(&db, photo_id).await?;
            }
        } else {
            for photo in &self.photos {
                self.overpass.update_image_places(&db, *photo).await?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct OverpassOpt {
    /// How to connect to the overpass API.
    ///
    /// See https://wiki.openstreetmap.org/wiki/Overpass_API for
    /// available servers and policies.
    #[structopt(long, env = "OVERPASS_URL")]
    overpass_url: String,
}

impl OverpassOpt {
    pub async fn update_image_places(
        &self,
        db: &PgPool,
        image: i32,
    ) -> Result<(), Error> {
        use crate::schema::positions::dsl::*;
        let coord = positions
            .filter(photo_id.eq(image))
            .select((latitude, longitude))
            .first::<Coord>(
                &db.get().map_err(|e| Error::Pool(image, e.to_string()))?,
            )
            .optional()
            .map_err(|e| Error::Db(image, e))?
            .ok_or_else(|| Error::NoPosition(image))?;
        debug!("Should get places for #{} at {:?}", image, coord);
        let data = Client::new()
            .post(&self.overpass_url)
            .body(format!("[out:json];is_in({},{});out;", coord.x, coord.y))
            .send()
            .await
            .and_then(Response::error_for_status)
            .map_err(|e| Error::Server(image, e))?
            .json::<Value>()
            .await
            .map_err(|e| Error::Server(image, e))?;

        if let Some(elements) = data
            .as_object()
            .and_then(|o| o.get("elements"))
            .and_then(Value::as_array)
        {
            let c = db.get().map_err(|e| Error::Pool(image, e.to_string()))?;
            for obj in elements {
                if let (Some(t_osm_id), Some((name, level))) =
                    (osm_id(obj), name_and_level(obj))
                {
                    debug!("{}: {} (level {})", t_osm_id, name, level);
                    let place = get_or_create_place(&c, t_osm_id, name, level)
                        .map_err(|e| Error::Db(image, e))?;
                    if place.osm_id.is_none() {
                        debug!("Matched {:?} by name, update osm info", place);
                        use crate::schema::places::dsl::*;
                        diesel::update(places)
                            .filter(id.eq(place.id))
                            .set((
                                osm_id.eq(Some(t_osm_id)),
                                osm_level.eq(level),
                            ))
                            .execute(&c)
                            .map_err(|e| Error::Db(image, e))?;
                    }
                    use crate::models::PhotoPlace;
                    use crate::schema::photo_places::dsl::*;
                    let q = photo_places
                        .filter(photo_id.eq(image))
                        .filter(place_id.eq(place.id));
                    if q.first::<PhotoPlace>(&c).is_ok() {
                        debug!(
                            "Photo #{} already has {} ({})",
                            image, place.id, place.place_name
                        );
                    } else {
                        diesel::insert_into(photo_places)
                            .values((
                                photo_id.eq(image),
                                place_id.eq(place.id),
                            ))
                            .execute(&c)
                            .map_err(|e| Error::Db(image, e))?;
                    }
                } else {
                    info!("Unused area: {}", obj);
                }
            }
        }
        Ok(())
    }
}

fn osm_id(obj: &Value) -> Option<i64> {
    obj.get("id").and_then(Value::as_i64)
}

fn name_and_level(obj: &Value) -> Option<(&str, i16)> {
    if let Some(tags) = obj.get("tags") {
        let name = tags
            .get("name:sv")
            //.or_else(|| tags.get("name:en"))
            .or_else(|| tags.get("name"))
            .and_then(Value::as_str);
        let level = tags
            .get("admin_level")
            .and_then(Value::as_str)
            .and_then(|l| l.parse().ok())
            .or_else(|| match tag_str(tags, "leisure") {
                Some("garden") => Some(18),
                Some("nature_reserve") => Some(12),
                Some("park") => Some(14),
                Some("pitch") => Some(15),
                Some("playground") => Some(16),
                _ => None,
            })
            .or_else(|| match tag_str(tags, "tourism") {
                Some("attraction") => Some(16),
                Some("theme_park") | Some("zoo") => Some(14),
                _ => None,
            })
            .or_else(|| match tag_str(tags, "boundary") {
                Some("national_park") => Some(14),
                Some("historic") => Some(7), // Seems to be mainly "Landskap"
                _ => None,
            })
            .or_else(|| match tag_str(tags, "landuse") {
                Some("allotments") => Some(14),
                Some("commercial") => Some(12),
                Some("grass") => Some(13),
                Some("industrial") => Some(11),
                Some("meadow") => Some(16),
                Some("railway") => Some(13),
                Some("residential") => Some(11),
                Some("retail") => Some(13),
                _ => None,
            })
            .or_else(|| match tag_str(tags, "highway") {
                Some("pedestrian") => Some(15),  // torg
                Some("residential") => Some(15), // torg?
                Some("rest_area") => Some(16),
                _ => None,
            })
            .or_else(|| match tag_str(tags, "public_transport") {
                Some("station") => Some(18),
                _ => None,
            })
            .or_else(|| match tag_str(tags, "amenity") {
                Some("bus_station") => Some(16),
                Some("exhibition_center") => Some(20),
                Some("kindergarten") => Some(15),
                Some("place_of_worship") => Some(15),
                Some("school") => Some(14),
                Some("university") => Some(12),
                _ => None,
            })
            .or_else(|| match tag_str(tags, "aeroway") {
                Some("aerodrome") => Some(14),
                _ => None,
            })
            .or_else(|| match tag_str(tags, "water") {
                Some("lake") => Some(15),
                _ => None,
            })
            .or_else(|| match tag_str(tags, "waterway") {
                Some("riverbank") => Some(16),
                _ => None,
            })
            .or_else(|| match tag_str(tags, "man_made") {
                Some("bridge") => Some(17),
                _ => None,
            })
            .or_else(|| match tag_str(tags, "place") {
                Some("city_block") => Some(17),
                Some("island") => Some(13),
                Some("islet") => Some(17),
                Some("penisula") => Some(13),
                Some("region") => Some(8),
                Some("square") => Some(18),
                Some("suburb") => Some(11),
                _ => None,
            })
            .or_else(|| match tag_str(tags, "natural") {
                Some("bay") => Some(14),
                Some("beach") => Some(15),
                Some("scrub") => Some(18),
                Some("wood") => Some(14),
                _ => None,
            })
            .or_else(|| match tag_str(tags, "building") {
                Some("exhibition_center") => Some(19),
                Some("sports_hall") => Some(19),
                Some(_) => Some(20),
                _ => None,
            })
            .or_else(|| match tag_str(tags, "political_division") {
                Some("canton") => Some(9),
                _ => None,
            });
        if let (Some(name), Some(level)) = (name, level) {
            debug!("{} is level {}", name, level);
            Some((name, level))
        } else {
            None
        }
    } else {
        warn!("Tag-less object {:?}", obj);
        None
    }
}

fn tag_str<'a>(tags: &'a Value, name: &str) -> Option<&'a str> {
    tags.get(name).and_then(Value::as_str)
}

fn get_or_create_place(
    c: &PgConnection,
    t_osm_id: i64,
    name: &str,
    level: i16,
) -> Result<Place, diesel::result::Error> {
    use crate::schema::places::dsl::*;
    places
        .filter(
            osm_id
                .eq(Some(t_osm_id))
                .or(place_name.eq(name).and(osm_id.is_null())),
        )
        .first::<Place>(c)
        .or_else(|_| {
            let mut result = diesel::insert_into(places)
                .values((
                    place_name.eq(&name),
                    slug.eq(&slugify(&name)),
                    osm_id.eq(Some(t_osm_id)),
                    osm_level.eq(Some(level)),
                ))
                .get_result::<Place>(c);
            let mut attempt = 1;
            while is_duplicate(&result) && attempt < 25 {
                info!("Attempt #{} got {:?}, trying again", attempt, result);
                attempt += 1;
                let name = format!("{} ({})", name, attempt);
                result = diesel::insert_into(places)
                    .values((
                        place_name.eq(&name),
                        slug.eq(&slugify(&name)),
                        osm_id.eq(Some(t_osm_id)),
                        osm_level.eq(Some(level)),
                    ))
                    .get_result::<Place>(c);
            }
            result
        })
}

fn is_duplicate<T>(r: &Result<T, DieselError>) -> bool {
    match r {
        Err(DieselError::DatabaseError(
            DatabaseErrorKind::UniqueViolation,
            _,
        )) => true,
        _ => false,
    }
}

#[derive(Debug)]
pub enum Error {
    NoPosition(i32),
    Db(i32, diesel::result::Error),
    Pool(i32, String),
    Server(i32, reqwest::Error),
}
