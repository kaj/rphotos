use adm::result::Error;
use chrono::naive::NaiveDateTime;
use diesel;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use models::{Modification, Person, Photo, Place, Tag};
use std::fs::File;
use std::path::Path;
use std::result;
use xml::attribute::OwnedAttribute;
use xml::reader::{EventReader, XmlEvent};

type Result<T> = result::Result<T, Error>;

pub fn readkpa(db: &PgConnection, dir: &Path) -> Result<()> {
    let file = File::open(dir.join("index.xml"))?;
    info!("Reading kphotoalbum data");
    let mut xml = EventReader::new(file);
    let mut option: Option<String> = None;
    let mut photo: Option<Photo> = None;
    while let Ok(event) = xml.next() {
        match event {
            XmlEvent::EndDocument => {
                debug!("End of xml");
                break;
            }
            XmlEvent::StartElement { ref name,
                                     ref attributes,
                                     ref namespace } => {
                debug!("Found {} {:?} {:?}", name, attributes, namespace);
                match &*name.local_name {
                    "image" => {
                        if let Some(file) = find_attr("file", attributes) {
                            let angle = find_attr("angle", attributes)
                                             .unwrap_or("0")
                                             .parse::<i16>()?;
                            let date = find_image_date(attributes);
                            match photo_by_path(db, file, date, angle) {
                                Ok(p) => { photo = Some(p) }
                                Err(e) => {
                                    error!("{}", e);
                                    photo = None;
                                }
                            }
                        }
                    }
                    "option" => {
                        option = find_attr("name", attributes)
                            .map(|s| s.into());
                    }
                    "value" => {
                        match (photo.as_mut(),
                                    option.as_ref().map(|s| s.as_ref()),
                                    find_attr("value", attributes)) {
                            (Some(p), Some("Nyckelord"), Some(v)) => {
                                tag_photo(db, p, v)
                            }
                            (Some(p), Some("Personer"), Some(v)) => {
                                person_photo(db, p, v)
                            }
                            (Some(p), Some("Platser"), Some(v)) => {
                                place_photo(db, p, v)
                            }
                            (Some(p), Some("Betyg"), Some(v)) => {
                                grade_photo(db, p, v)
                            }
                            (None, _option, _value_in_categories) => Ok(()),
                            (p, o, v) => {
                                Err(Error::Other(format!("Got value {:?} \
                                                          for option {:?} \
                                                          on photo {:?}",
                                                         v,
                                                         o,
                                                         p)))
                            }
                        }?
                    }
                    _ => {}
                }
            }
            XmlEvent::EndElement { ref name } => {
                if name.local_name == "option" {
                    option = None;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn photo_by_path(db: &PgConnection,
                     file: &str,
                     date: Option<NaiveDateTime>,
                     angle: i16)
                     -> Result<Photo> {
    match Photo::update_by_path(db, file, date, angle, &None)? {
        Some(Modification::Created(photo)) => {
            info!("Created {:?}", photo);
            Ok(photo)
        }
        Some(Modification::Updated(photo)) => {
            info!("Modified {:?}", photo);
            Ok(photo)
        }
        Some(Modification::Unchanged(photo)) => {
            debug!("No change for {:?}", photo);
            Ok(photo)
        }
        None => {
            let lower_path = file.to_lowercase();
            if lower_path != file {
                photo_by_path(db, &lower_path, date, angle)
            } else {
                Err(Error::Other(format!("Photo {:?} does not exist in db.",
                                         file)))
            }
        }
    }
}

fn tag_photo(db: &PgConnection, thephoto: &Photo, tagname: &str) -> Result<()> {
    use models::{NewPhotoTag, NewTag, PhotoTag};
    let tag = {
        use schema::tags::dsl::*;
        tags.filter(tag_name.eq(tagname))
            .first::<Tag>(db)
            .or_else(|_| {
                diesel::insert(&NewTag {
                        tag_name: tagname,
                        slug: &slugify(tagname),
                    })
                    .into(tags)
                    .get_result::<Tag>(db)
            })
    }?;
    debug!("  tag {:?}", tag);
    use schema::photo_tags::dsl::*;
    let q = photo_tags.filter(photo_id.eq(thephoto.id))
        .filter(tag_id.eq(tag.id));
    if let Ok(result) = q.first::<PhotoTag>(db) {
        debug!("  match {:?}", result)
    } else {
        debug!("  new tag {:?} on {:?}!", tag, thephoto);
        diesel::insert(&NewPhotoTag {
                photo_id: thephoto.id,
                tag_id: tag.id,
            })
            .into(photo_tags)
            .execute(db)
            .expect("Tag a photo");
    }
    Ok(())
}

fn person_photo(db: &PgConnection, photo: &Photo, name: &str) -> Result<()> {
    use models::{NewPerson, NewPhotoPerson, PhotoPerson};
    let person = {
        use schema::people::dsl::*;
        people.filter(person_name.eq(name))
            .first::<Person>(db)
            .or_else(|_| {
                diesel::insert(&NewPerson {
                        person_name: name,
                        slug: &slugify(name),
                    })
                    .into(people)
                    .get_result::<Person>(db)
            })
    }?;
    debug!("  person {:?}", person);
    use schema::photo_people::dsl::*;
    let q = photo_people.filter(photo_id.eq(photo.id))
        .filter(person_id.eq(person.id));
    if let Ok(result) = q.first::<PhotoPerson>(db) {
        debug!("  match {:?}", result);
    } else {
        debug!("  new person {:?} on {:?}!", person, photo);
        diesel::insert(&NewPhotoPerson {
                photo_id: photo.id,
                person_id: person.id,
            })
            .into(photo_people)
            .execute(db)
            .map_err(|e| {
                Error::Other(format!("Place photo {:?}: {}", photo, e))
            })?;
    }
    Ok(())
}

fn place_photo(db: &PgConnection, photo: &Photo, name: &str) -> Result<()> {
    use models::{NewPhotoPlace, NewPlace, PhotoPlace};
    let place = {
        use schema::places::dsl::*;
        places.filter(place_name.eq(name))
            .first::<Place>(db)
            .or_else(|_| {
                diesel::insert(&NewPlace {
                        place_name: name,
                        slug: &slugify(name),
                    })
                    .into(places)
                    .get_result::<Place>(db)
            })
    }?;
    debug!("  place {:?}", place);
    use schema::photo_places::dsl::*;
    photo_places.filter(photo_id.eq(photo.id))
        .filter(place_id.eq(place.id))
        .first::<PhotoPlace>(db)
        .map(|_| ())
        .or_else(|_| {
            debug!("  new place {:?} on {:?}!", place, photo);
            diesel::insert(&NewPhotoPlace {
                    photo_id: photo.id,
                    place_id: place.id,
                })
                .into(photo_places)
                .execute(db)
                .map(|_| ())
        })
        .map_err(|e| Error::Other(format!("Place photo {:?}: {}", photo, e)))
}

fn grade_photo(db: &PgConnection, photo: &mut Photo, name: &str) -> Result<()> {
    debug!("Should set  grade {:?} on {:?}", name, photo);
    photo.grade = Some(match name {
        "Usel" => 0,
        "Ok" => 3,
        "utvald" => 5,
        x => {
            return Err(Error::Other(format!("Unknown grade {:?} on {:?}",
                                            x,
                                            photo)))
        }
    });
    use schema::photos::dsl::*;
    let n = diesel::update(photos.find(photo.id))
        .set(grade.eq(photo.grade))
        .execute(db)
        .map_err(|e| {
            Error::Other(format!("Update grade of {:?}: {}", photo, e))
        })?;
    debug!("Graded {} photo", n);
    Ok(())
}

fn slugify(val: &str) -> String {
    val.chars()
        .map(|c| match c {
            c @ '0'...'9' | c @ 'a'...'z'=> c,
            c @ 'A'...'Z' => (c as u8 - b'A' + b'a') as char,
            'Å' | 'å' | 'Ä' | 'ä' => 'a',
            'Ö' | 'ö' | 'Ô' | 'ô' => 'o',
            'É' | 'é' | 'Ë' | 'ë' | 'Ê' | 'ê' => 'e',
            'Ü' | 'ü' | 'Û' | 'û' => 'u',
            _ => '_',
        })
        .collect()
}

fn find_image_date(attributes: &[OwnedAttribute]) -> Option<NaiveDateTime> {
    let start_date = find_attr("startDate", attributes).unwrap_or("");
    let end_date = find_attr("endDate", attributes).unwrap_or("");
    let format = "%FT%T";
    if let Ok(start_t) = NaiveDateTime::parse_from_str(&*start_date, format) {
        if let Ok(end_t) = NaiveDateTime::parse_from_str(&*end_date, format) {
            if start_t != end_t {
                println!("Found interval {} - {}", start_t, end_t);
                Some(end_t)
            } else {
                Some(start_t)
            }
        } else {
            Some(start_t)
        }
    } else {
        if let Ok(end_t) = NaiveDateTime::parse_from_str(&*end_date, format) {
            Some(end_t)
        } else {
            None
        }
    }
}

fn find_attr<'a>(name: &str, attrs: &'a [OwnedAttribute]) -> Option<&'a str> {
    for attr in attrs {
        if attr.name.local_name == name {
            return Some(&attr.value);
        }
    }
    None
}
