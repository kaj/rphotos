#[macro_use]
extern crate log;
extern crate xml;
extern crate rustc_serialize;
extern crate env_logger;
extern crate time;
extern crate chrono;
extern crate diesel;
extern crate rphotos;
extern crate dotenv;

use chrono::naive::datetime::NaiveDateTime;
use std::fs::File;
use xml::attribute::OwnedAttribute;
use xml::reader::EventReader;
use xml::reader::XmlEvent; // ::{EndDocument, StartElement};
use diesel::pg::PgConnection;
use rphotos::models::{Modification, Person, Photo, Place, Tag};
use dotenv::dotenv;
use self::diesel::prelude::*;

mod env;
use env::{dburl, photos_dir};

fn find_attr(name: &str, attrs: &Vec<OwnedAttribute>) -> Option<String> {
    for attr in attrs {
        if attr.name.local_name == name {
            return Some(attr.value.clone());
        }
    }
    None
}

fn slugify(val: &str) -> String {
    val.chars()
       .map(|c| match c {
           c @ '0'...'9' => c,
           c @ 'a'...'z' => c,
           c @ 'A'...'Z' => c.to_lowercase().next().unwrap(),
           'Å' | 'å' | 'Ä' | 'ä' => 'a',
           'Ö' | 'ö' => 'o',
           'É' | 'é' => 'e',
           _ => '_',
       })
       .collect()
}

fn tag_photo(db: &PgConnection, thephoto: &Photo, tagname: &str) {
    use rphotos::models::{NewTag, PhotoTag};
    /*
    let ttag = {
        use rphotos::schema::tag::dsl::*;
        if let Ok(ttag) = tag.filter(tag_name.eq(tagname)).first::<Tag>(db) {
            ttag
        } else {
            diesel::insert(&NewTag {
                tag_name: tagname,
                slug: &slugify(tagname),
            }).into(tag).get_result::<Tag>(db).expect("Insert new tag")
        }
    };
    //        get_or_create(db, "tag", &tag, &[("slug", &slugify(tag))]);
    debug!("  tag {:?}", tag);

    use rphotos::schema::photo_tag::dsl::*;
    let q = photo_tag.filter(photo.eq(thephoto.id)).filter(tag.eq(ttag.id));
    if let Ok(result) = q.first::<PhotoTag>(db) {
        debug!("  match {:?}", result)
    } else {
        debug!("  new tag {:?} on {:?}!", tag, photo);
        / * TODO
        let mut q = Query::insert();
        q.into_table("public.photo_tag");
        q.set("photo", &photo.id);
        q.set("tag", &tag.id);
        q.execute(db).unwrap();
         * /
    }
    */
}

fn person_photo(db: &PgConnection, photo: &Photo, name: &str) {
    /*
    let person: Person = get_or_create(db,
                                       "name",
                                       &name,
                                       &[("slug", &slugify(name))]);
    debug!("  person {:?}", person);
    let mut q = Query::select();
    q.from_table("public.photo_person");
    q.filter_eq("photo", &photo.id);
    q.filter_eq("person", &person.id);
    if let Ok(Some(result)) = q.retrieve_one(db) {
        debug!("  match {:?}", result)
    } else {
        println!("  new person {:?} on {:?}!", person, photo);
        let mut q = Query::insert();
        q.into_table("public.photo_person");
        q.set("photo", &photo.id);
        q.set("person", &person.id);
        q.execute(db).unwrap();
    }
     */
}

fn place_photo(db: &PgConnection, photo: &Photo, name: &str) {
    /*
    let place: Place = get_or_create(db,
                                     "place",
                                     &name,
                                     &[("slug", &slugify(name))]);
    debug!("  place {:?}", place);
    let mut q = Query::select();
    q.from_table("public.photo_place");
    q.filter_eq("photo", &photo.id);
    q.filter_eq("place", &place.id);
    if let Ok(Some(result)) = q.retrieve_one(db) {
        debug!("  match {:?}", result)
    } else {
        println!("  new place {:?} on {:?}!", place, photo);
        let mut q = Query::insert();
        q.into_table("public.photo_place");
        q.set("photo", &photo.id);
        q.set("place", &place.id);
        q.execute(db).unwrap();
    }
*/
}

fn grade_photo(db: &PgConnection, photo: &mut Photo, name: &str) {
    debug!("Should set  grade {:?} on {:?}", name, photo);
    photo.grade = Some(match name {
        "Usel" => 0,
        "Ok" => 3,
        "utvald" => 5,
        x => panic!("Unknown grade {:?} on {:?}", x, photo),
    });
    use rphotos::schema::photos::dsl::*;
    let n = diesel::update(photos.find(photo.id))
        .set(grade.eq(photo.grade))
        .execute(db).expect(&format!("Update grade of {:?}", photo));
    debug!("Graded {} photo", n);
}

fn main() {
    dotenv().ok();
    env_logger::init().unwrap();
    let db = PgConnection::establish(&dburl())
        .expect("Error connecting to database");
    let file = File::open(photos_dir().join("index.xml")).unwrap();
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
                                            .unwrap_or("0".to_string())
                                            .parse::<i16>()
                                            .unwrap();
                            let date = find_image_date(attributes);
                            photo = Some(match Photo::create_or_set_basics
                                (&db, &file, date, angle)
                                .expect("Create or update photo") {
                                    Modification::Created(photo) => {
                                        info!("Created {:?}", photo);
                                        photo
                                    }
                                    Modification::Updated(photo) => {
                                        info!("Modified {:?}", photo);
                                        photo
                                    }
                                    Modification::Unchanged(photo) => {
                                        debug!("No change for {:?}", photo);
                                        photo
                                    },
                                })
                        }
                    }
                    "option" => {
                        option = find_attr("name", attributes);
                    }
                    "value" => {
                        if let Some(ref o) = option {
                            if let Some(v) = find_attr("value", attributes) {
                                match &**o {
                                    "Nyckelord" => {
                                        if let Some(ref photo) = photo {
                                            tag_photo(&db, &photo, &v);
                                        }
                                    }
                                    "Personer" => {
                                        if let Some(ref photo) = photo {
                                            person_photo(&db,
                                                         &photo,
                                                         &v);
                                        }
                                    }
                                    "Platser" => {
                                        if let Some(ref photo) = photo {
                                            place_photo(&db,
                                                        &photo,
                                                        &v);
                                        }
                                    }
                                    "Betyg" => {
                                        if let Some(ref mut photo) = photo {
                                            grade_photo(&db, photo, &v);
                                        }
                                    }
                                    o => {
                                        warn!("Unsupported metadata {} = {}",
                                              o,
                                              v);
                                    }
                                }

                            }
                        }
                    }
                    _ => {}
                }
            }
            XmlEvent::EndElement { ref name } => {
                match &*name.local_name {
                    "option" => {
                        option = None;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn find_image_date(attributes: &Vec<OwnedAttribute>) -> Option<NaiveDateTime> {
    let start_date = find_attr("startDate", attributes)
                         .unwrap_or("".to_string());
    let end_date = find_attr("endDate", attributes).unwrap_or("".to_string());
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
