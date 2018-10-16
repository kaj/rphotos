use diesel;
use diesel::prelude::*;
use models::Coord;
use reqwest::Client;
use rustc_serialize::json::Json;
use slug::slugify;

pub fn update_image_places(
    c: &PgConnection,
    image: i32,
) -> Result<(), String> {
    use schema::positions::dsl::*;
    let coord = match positions
        .filter(photo_id.eq(image))
        .select((latitude, longitude))
        .first::<(i32, i32)>(c)
    {
        Ok((tlat, tlong)) => Coord {
            x: f64::from(tlat) / 1e6,
            y: f64::from(tlong) / 1e6,
        },
        Err(diesel::NotFound) => {
            return Err(format!(
                "Image #{} does not exist or has no position",
                image
            ));
        }
        Err(err) => {
            return Err(format!("Failed to get image position: {}", err));
        }
    };
    debug!("Should get places for {:?}", coord);
    let client = Client::new();
    match client
        .post("https://overpass.kumi.systems/api/interpreter")
        .body(format!("[out:json];is_in({},{});out;", coord.x, coord.y))
        .send()
    {
        Ok(mut response) => {
            if response.status().is_success() {
                let data = Json::from_reader(&mut response).unwrap();
                let obj = data.as_object().unwrap();
                if let Some(elements) =
                    obj.get("elements").and_then(|o| o.as_array())
                {
                    for obj in elements {
                        if let (Some(t_osm_id), Some((name, level))) =
                            (osm_id(obj), name_and_level(obj))
                        {
                            debug!("{}: {} (level {})", t_osm_id, name, level);
                            let place = {
                                use models::Place;
                                use schema::places::dsl::*;
                                places
                                    .filter(
                                        osm_id.eq(Some(t_osm_id)).or(
                                            place_name
                                                .eq(name)
                                                .and(osm_id.is_null()),
                                        ),
                                    )
                                    .first::<Place>(c)
                                    .or_else(|_| {
                                        diesel::insert_into(places)
                                            .values((
                                                place_name.eq(&name),
                                                slug.eq(&slugify(&name)),
                                                osm_id.eq(Some(t_osm_id)),
                                                osm_level.eq(Some(level)),
                                            ))
                                            .get_result::<Place>(c)
                                            .or_else(|_| {
                                                let name = format!(
                                                    "{} ({})",
                                                    name, level
                                                );
                                                diesel::insert_into(places)
                                                    .values((
                                                        place_name.eq(&name),
                                                        slug.eq(&slugify(
                                                            &name,
                                                        )),
                                                        osm_id.eq(Some(
                                                            t_osm_id,
                                                        )),
                                                        osm_level
                                                            .eq(Some(level)),
                                                    ))
                                                    .get_result::<Place>(c)
                                            })
                                    })
                                    .expect("Find or create place")
                            };
                            if place.osm_id.is_none() {
                                debug!(
                                    "Matched {:?} by name, update osm info",
                                    place
                                );
                                use schema::places::dsl::*;
                                diesel::update(places)
                                    .filter(id.eq(place.id))
                                    .set((
                                        osm_id.eq(Some(t_osm_id)),
                                        osm_level.eq(level),
                                    ))
                                    .execute(c)
                                    .expect(&format!(
                                        "Update OSM for {:?}",
                                        place
                                    ));
                            }
                            use models::PhotoPlace;
                            use schema::photo_places::dsl::*;
                            let q = photo_places
                                .filter(photo_id.eq(image))
                                .filter(place_id.eq(place.id));
                            if q.first::<PhotoPlace>(c).is_ok() {
                                debug!(
                                    "Photo #{} already has {:?}",
                                    image, place.id
                                );
                            } else {
                                diesel::insert_into(photo_places)
                                    .values((
                                        photo_id.eq(image),
                                        place_id.eq(place.id),
                                    ))
                                    .execute(c)
                                    .expect("Place a photo");
                            }
                        }
                    }
                }
            } else {
                warn!("Bad response from overpass: {:?}", response);
            }
        }
        Err(err) => {
            warn!("Failed to get overpass info: {}", err);
        }
    }

    Ok(())
}

fn osm_id(obj: &Json) -> Option<i64> {
    obj.find("id").and_then(|o| o.as_i64())
}

fn name_and_level(obj: &Json) -> Option<(&str, i16)> {
    obj.find("tags").and_then(|tags| {
        let name = tags
            .find("name:sv")
            //.or_else(|| tags.find("name:en"))
            .or_else(|| tags.find("name"))
            .and_then(|o| o.as_string());
        let level = tags
            .find("admin_level")
            .and_then(|o| o.as_string())
            .and_then(|s| s.parse().ok())
            .or_else(|| {
                match tags.find("leisure").and_then(|o| o.as_string()) {
                    Some("garden") => Some(18),
                    Some("nature_reserve") => Some(12),
                    Some("park") => Some(14),
                    Some("playground") => Some(16),
                    _ => None,
                }
            })
            .or_else(|| {
                match tags.find("tourism").and_then(|o| o.as_string()) {
                    Some("attraction") => Some(16),
                    Some("theme_park") | Some("zoo") => Some(14),
                    _ => None,
                }
            })
            .or_else(|| {
                match tags.find("boundary").and_then(|o| o.as_string()) {
                    Some("national_park") => Some(14),
                    _ => None,
                }
            })
            .or_else(|| {
                match tags.find("building").and_then(|o| o.as_string()) {
                    Some("church") => Some(20),
                    Some("exhibition_center") => Some(20),
                    Some("industrial") => Some(20),
                    Some("office") => Some(20),
                    Some("public") => Some(20),
                    Some("retail") => Some(20),
                    Some("university") => Some(20),
                    Some("yes") => Some(20),
                    _ => None,
                }
            })
            .or_else(|| {
                match tags.find("landuse").and_then(|o| o.as_string()) {
                    Some("industrial") => Some(11),
                    Some("residential") => Some(11),
                    _ => None,
                }
            })
            .or_else(|| {
                match tags.find("highway").and_then(|o| o.as_string()) {
                    Some("pedestrian") => Some(15), // torg
                    Some("rest_area") => Some(16),
                    _ => None,
                }
            })
            .or_else(|| {
                match tags.find("public_transport").and_then(|o| o.as_string())
                {
                    Some("station") => Some(18),
                    _ => None,
                }
            })
            .or_else(|| {
                match tags.find("amenity").and_then(|o| o.as_string()) {
                    Some("exhibition_center") => Some(20),
                    Some("place_of_worship") => Some(15),
                    Some("university") => Some(12),
                    _ => None,
                }
            });
        if let (Some(name), Some(level)) = (name, level) {
            info!("{} is level {}", name, level);
            Some((name, level))
        } else {
            info!("Unused area {}", obj);
            None
        }
    })
}
