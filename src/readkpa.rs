#[macro_use] extern crate log;
extern crate xml;
extern crate rustorm;
extern crate rustc_serialize;

use rustorm::database::Database;
use rustorm::pool::ManagedPool;
use rustorm::query::Query;
use std::fs::File;
use xml::attribute::OwnedAttribute;
use xml::reader::EventReader;
use xml::reader::XmlEvent; // ::{EndDocument, StartElement};

mod models;
use models::{Photo, Tag, get_or_create, get_or_create_default};

mod env;
use env::dburl;

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

    let tag: Tag = get_or_create_default(db, "tag", &tag,
                                         &[("slug", &slugify(v2))]);
    info!("  tag {:?}", tag);
    let mut q = Query::select();
    q.from_table("public.photo_tag");
    q.filter_eq("photo", &photo.id);
    q.filter_eq("tag", &tag.id);
    if let Ok(Some(result)) = q.retrieve_one(db) {
        debug!("  match {:?}", result)
    } else {
        info!("  new tag {:?} on {:?}!", tag, photo);
        let mut q = Query::insert();
        q.into_table("public.photo_tag");
        q.set("photo", &photo.id);
        q.set("tag", &tag.id);
        q.execute(db);
    }
}

fn main() {
    let pool = ManagedPool::init(&dburl(), 1).unwrap();
    let db = pool.connect().unwrap();
    let file = File::open("/home/kaj/Bilder/foto/index.xml").unwrap();
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
                            let img = get_or_create::<Photo>(db.as_ref(), "path", &file);
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
                                    o => { debug!("  {} = {}", o, v); }
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
