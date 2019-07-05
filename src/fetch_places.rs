use crate::models::{Coord, Place};
use crate::DbOpt;
use diesel;
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
    pub fn run(&self) -> Result<(), super::adm::result::Error> {
        let db = self.db.connect()?;
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
                .load::<(i32, Coord)>(&db)?;
            for (photo_id, coord) in result {
                println!("Find places for #{}, {:?}", photo_id, coord);
                update_image_places(&db, photo_id)?;
            }
        } else {
            for photo in &self.photos {
                update_image_places(&db, *photo)?;
            }
        }
        Ok(())
    }
}

pub fn update_image_places(c: &PgConnection, image: i32) -> Result<(), Error> {
    use crate::schema::positions::dsl::*;
    let coord = positions
        .filter(photo_id.eq(image))
        .select((latitude, longitude))
        .first::<Coord>(c)
        .optional()
        .map_err(|e| Error::Db(image, e))?
        .ok_or_else(|| Error::NoPosition(image))?;
    debug!("Should get places for #{} at {:?}", image, coord);
    let data = Client::new()
        .post("https://overpass.kumi.systems/api/interpreter")
        .body(format!("[out:json];is_in({},{});out;", coord.x, coord.y))
        .send()
        .and_then(Response::error_for_status)
        .and_then(|mut r| r.json::<Value>())
        .map_err(|e| Error::Server(image, e))?;

    if let Some(elements) = data
        .as_object()
        .and_then(|o| o.get("elements"))
        .and_then(Value::as_array)
    {
        for obj in elements {
            if let (Some(t_osm_id), Some((name, level))) =
                (osm_id(obj), name_and_level(obj))
            {
                debug!("{}: {} (level {})", t_osm_id, name, level);
                let place = get_or_create_place(c, t_osm_id, name, level)
                    .map_err(|e| Error::Db(image, e))?;
                if place.osm_id.is_none() {
                    debug!("Matched {:?} by name, update osm info", place);
                    use crate::schema::places::dsl::*;
                    diesel::update(places)
                        .filter(id.eq(place.id))
                        .set((osm_id.eq(Some(t_osm_id)), osm_level.eq(level)))
                        .execute(c)
                        .map_err(|e| Error::Db(image, e))?;
                }
                use crate::models::PhotoPlace;
                use crate::schema::photo_places::dsl::*;
                let q = photo_places
                    .filter(photo_id.eq(image))
                    .filter(place_id.eq(place.id));
                if q.first::<PhotoPlace>(c).is_ok() {
                    debug!(
                        "Photo #{} already has {} ({})",
                        image, place.id, place.place_name
                    );
                } else {
                    diesel::insert_into(photo_places)
                        .values((photo_id.eq(image), place_id.eq(place.id)))
                        .execute(c)
                        .map_err(|e| Error::Db(image, e))?;
                }
            } else {
                info!("Unused area: {}", obj);
            }
        }
    }
    Ok(())
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
            .or_else(|| match tags.get("leisure").and_then(Value::as_str) {
                Some("garden") => Some(18),
                Some("nature_reserve") => Some(12),
                Some("park") => Some(14),
                Some("playground") => Some(16),
                _ => None,
            })
            .or_else(|| match tags.get("tourism").and_then(Value::as_str) {
                Some("attraction") => Some(16),
                Some("theme_park") | Some("zoo") => Some(14),
                _ => None,
            })
            .or_else(|| match tags.get("boundary").and_then(Value::as_str) {
                Some("national_park") => Some(14),
                Some("historic") => Some(7), // Seems to be mainly "Landskap"
                _ => None,
            })
            .or_else(|| match tags.get("building").and_then(Value::as_str) {
                Some("church") => Some(20),
                Some("exhibition_center") => Some(20),
                Some("industrial") => Some(20),
                Some("office") => Some(20),
                Some("public") => Some(20),
                Some("retail") => Some(20),
                Some("university") => Some(20),
                Some("yes") => Some(20),
                _ => None,
            })
            .or_else(|| match tags.get("landuse").and_then(Value::as_str) {
                Some("industrial") => Some(11),
                Some("residential") => Some(11),
                _ => None,
            })
            .or_else(|| match tags.get("highway").and_then(Value::as_str) {
                Some("pedestrian") => Some(15), // torg
                Some("rest_area") => Some(16),
                _ => None,
            })
            .or_else(|| {
                match tags.get("public_transport").and_then(Value::as_str) {
                    Some("station") => Some(18),
                    _ => None,
                }
            })
            .or_else(|| match tags.get("amenity").and_then(Value::as_str) {
                Some("exhibition_center") => Some(20),
                Some("bus_station") => Some(16),
                Some("place_of_worship") => Some(15),
                Some("university") => Some(12),
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
    Server(i32, reqwest::Error),
}
