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
        .body(format!(
            "[out:json];is_in({},{});area._[admin_level];out;",
            coord.x, coord.y,
        )).send()
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
                                    ).first::<Place>(c)
                                    .or_else(|_| {
                                        diesel::insert_into(places)
                                            .values((
                                                place_name.eq(&name),
                                                slug.eq(&slugify(&name)),
                                                osm_id.eq(Some(t_osm_id)),
                                                osm_level.eq(Some(level)),
                                            )).get_result::<Place>(c)
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
                                                    )).get_result::<Place>(c)
                                            })
                                    }).expect("Find or create place")
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
                                    )).execute(c)
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
                                    )).execute(c)
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
            .and_then(|s| s.parse().ok());
        if let (Some(name), Some(level)) = (name, level) {
            Some((name, level))
        } else {
            None
        }
    })
}
