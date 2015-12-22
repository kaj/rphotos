#[macro_use] extern crate log;
extern crate xml;
extern crate rustorm;
extern crate rustc_serialize;
extern crate env_logger;
extern crate time;
extern crate chrono;

use chrono::datetime::DateTime;
use chrono::naive::datetime::NaiveDateTime;
use chrono::offset::utc::UTC;
use chrono::offset::TimeZone;
use rustorm::database::Database;
use rustorm::pool::ManagedPool;
use rustorm::query::Query;
use rustorm::dao::ToValue;
use rustorm::table::IsTable;
use std::fs::File;
use xml::attribute::OwnedAttribute;
use xml::reader::EventReader;
use xml::reader::XmlEvent; // ::{EndDocument, StartElement};

mod models;
use models::{Photo, Tag, Person, Place, get_or_create};

mod env;
use env::{dburl, photos_dir};

fn find_attr(name: &str, attrs: &Vec<OwnedAttribute>) -> Option<String> {
    for attr in attrs {
        if attr.name.local_name == name {
            return Some(attr.value.clone())
        }
    }
    None
}

fn slugify(val: String) -> String {
    val.chars().map(|c| match c {
        c @ '0' ... '9' => c,
        c @ 'a' ... 'z' => c,
        c @ 'A' ... 'Z' => c.to_lowercase().next().unwrap(),
        'Å' | 'å' | 'Ä' | 'ä' => 'a',
        'Ö' | 'ö' => 'o',
        'É' | 'é' => 'e',
        _ => '_'
    }).collect()
}

fn tag_photo(db: &Database, photo: &Photo, tag: String) {
    let v2 : String = tag.clone();

    let tag: Tag = get_or_create(db, "tag", &tag, &[("slug", &slugify(v2))]);
    debug!("  tag {:?}", tag);
    let mut q = Query::select();
    q.from_table("public.photo_tag");
    q.filter_eq("photo", &photo.id);
    q.filter_eq("tag", &tag.id);
    if let Ok(Some(result)) = q.retrieve_one(db) {
        debug!("  match {:?}", result)
    } else {
        debug!("  new tag {:?} on {:?}!", tag, photo);
        let mut q = Query::insert();
        q.into_table("public.photo_tag");
        q.set("photo", &photo.id);
        q.set("tag", &tag.id);
        q.execute(db).unwrap();
    }
}

fn person_photo(db: &Database, photo: &Photo, name: String) {
    let v2: String = name.clone();
    let person: Person = get_or_create(db, "name", &name,
                                       &[("slug", &slugify(v2))]);
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
}

fn place_photo(db: &Database, photo: &Photo, name: String) {
    let v2: String = name.clone();
    let place: Place = get_or_create(db, "place", &name,
                                     &[("slug", &slugify(v2))]);
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
}

fn grade_photo(db: &Database, photo: &mut Photo, name: String) {
    debug!("Should set  grade {:?} on {:?}", name, photo);
    let grade = match &*name {
        "Usel" => 0,
        "Ok" => 3,
        "utvald" => 5,
        x => panic!("Unknown grade {:?} on {:?}", x, photo)
    };
    photo.grade = Some(grade);
    let mut q = Query::update();
    q.from(&Photo::table());
    q.filter_eq("id", &photo.id);
    q.set("grade", &grade);
    let n = q.execute(db).unwrap();
    debug!("Graded {} photo", n);
}

fn main() {
    env_logger::init().unwrap();
    let pool = ManagedPool::init(&dburl(), 1).unwrap();
    let db = pool.connect().unwrap();
    let file = File::open(photos_dir().join("index.xml")).unwrap();
    info!("Reading kphotoalbum data");
    let mut xml = EventReader::new(file);
    let mut option : Option<String> = None;
    let mut photo : Option<Photo> = None;
    while let Ok(event) = xml.next() {
        match event {
            XmlEvent::EndDocument => {
                debug!("End of xml");
                break;
            },
            XmlEvent::StartElement{ref name, ref attributes, ref namespace} => {
                match &*name.local_name {
                    "image" => {
                        if let Some(file) = find_attr("file", attributes) {
                            let angle = find_attr("angle", attributes).unwrap_or("0".to_string()).parse::<i16>().unwrap();
                            let date = find_image_date(attributes);
                            let mut defaults: Vec<(&str, &ToValue)> =
                                vec![("rotation", &angle)];
                            if let Some(ref date) = date {
                                defaults.push(("date", date));
                            }
                            let img: Photo = get_or_create(db.as_ref(), "path", &file,
                                                           &defaults);
                            debug!("Found image {:?}", img);
                            photo = Some(img);
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
                                            tag_photo(db.as_ref(), &photo, v);
                                        }
                                    },
                                    "Personer" => {
                                        if let Some(ref photo) = photo {
                                            person_photo(db.as_ref(), &photo, v);
                                        }
                                    },
                                    "Platser" => {
                                        if let Some(ref photo) = photo {
                                            place_photo(db.as_ref(), &photo, v);
                                        }
                                    },
                                    "Betyg" => {
                                        if let Some(ref mut photo) = photo {
                                            grade_photo(db.as_ref(), photo, v);
                                        }
                                    }
                                    o => { warn!("Unsupported metadata {} = {}", o, v); }
                                }
                                
                            }
                        }
                    }
                    _ => {}
                }
            },
            XmlEvent::EndElement{ref name} => {
                match &*name.local_name {
                    "option" => { option = None; }
                    _ => {}
                }
            }
            _ => {
            }
        }
    }
}

fn find_image_date(attributes: &Vec<OwnedAttribute>) -> Option<DateTime<UTC>> {
    let start_date = find_attr("startDate", attributes).unwrap_or("".to_string());
    let end_date = find_attr("endDate", attributes).unwrap_or("".to_string());
    let format = "%FT%T";
    let utc : UTC = UTC;
    if let Ok(start_t) = NaiveDateTime::parse_from_str(&*start_date, format) {
        if let Ok(end_t) = NaiveDateTime::parse_from_str(&*end_date, format) {
            if start_t != end_t {
                println!("Found interval {} - {}", start_t, end_t);
                utc.from_local_datetime(&end_t).latest()
            } else {
                utc.from_local_datetime(&start_t).latest()
            }
        } else {
            utc.from_local_datetime(&start_t).latest()
        }
    } else {
        if let Ok(end_t) = NaiveDateTime::parse_from_str(&*end_date, format) {
            utc.from_local_datetime(&end_t).latest()
        } else {
            None
        }
    }
}
