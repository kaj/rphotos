use chrono::naive::datetime::NaiveDateTime;
use rustc_serialize::{Encodable, Encoder};
use diesel::prelude::*;
use diesel::pg::PgConnection;
use diesel::result::Error as DieselError;

pub const MIN_PUBLIC_GRADE: i16 = 4;

#[derive(Debug, Clone, Queryable)]
pub struct Photo {
    pub id: i32,
    pub path: String,
    pub date: Option<NaiveDateTime>,
    pub grade: Option<i16>,
    pub rotation: i16,
}

// NaiveDateTime isn't Encodable, so we have to implement this by hand.
impl Encodable for Photo {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        s.emit_struct("Photo", 3, |s| {
            try!(s.emit_struct_field("id", 0, |s| s.emit_i32(self.id)));
            try!(s.emit_struct_field("path", 1, |s| s.emit_str(&self.path)));
            try!(s.emit_struct_field("date", 2, |s|
                s.emit_str(&self.date.map(|d|format!("{:?}", d))
                           .unwrap_or("-".to_string()))
            ));
            try!(s.emit_struct_field("grade", 2, |s| match self.grade {
                Some(g) => s.emit_option_some(|s| s.emit_i16(g)),
                None => s.emit_option_none(),
            }));
            s.emit_struct_field("rotation", 2, |s| s.emit_i16(self.rotation))
        })
    }
}

use super::schema::photos;
#[insertable_into(photos)]
#[derive(Debug, Clone)]
pub struct NewPhoto<'a> {
    pub path: &'a str,
    pub date: Option<NaiveDateTime>,
    pub rotation: i16,
}

#[derive(Debug)]
pub enum Modification<T> {
    Created(T),
    Updated(T),
    Unchanged(T),
}

impl Photo {
    #[allow(dead_code)]
    pub fn is_public(&self) -> bool {
        if let Some(grade) = self.grade {
            grade >= MIN_PUBLIC_GRADE
        } else {
            false
        }
    }

    pub fn create_or_set_basics(db: &PgConnection, file_path: &str,
                                exifdate: Option<NaiveDateTime>, exifrotation: i16)
                                -> Result<Modification<Photo>, DieselError> {
        use diesel;
        use diesel::prelude::*;
        use schema::photos::dsl::*;
        if let Some(mut pic) =
            try!(photos.filter(path.eq(&file_path.to_string()))
                       .first::<Photo>(db)
                       .optional()) {
            let mut change = false;
            if exifdate.is_some() && exifdate != pic.date {
                change = true;
                pic = try!(diesel::update(photos.find(pic.id))
                           .set(date.eq(exifdate))
                           .get_result::<Photo>(db));
            }
            if exifrotation != pic.rotation {
                change = true;
                pic = try!(diesel::update(photos.find(pic.id))
                           .set(rotation.eq(exifrotation))
                           .get_result::<Photo>(db));
            }
            Ok(if change { Modification::Updated(pic) }
               else { Modification::Unchanged(pic) })
        } else {
            let pic = NewPhoto {
                path: &file_path,
                date: exifdate,
                rotation: exifrotation,
            };
            let pic = try!(diesel::insert(&pic).into(photos)
                           .get_result::<Photo>(db));
            Ok(Modification::Created(pic))
        }
    }
}

#[derive(Debug, Clone, RustcEncodable, Queryable)]
pub struct Tag {
    pub id: i32,
    pub slug: String,
    pub tag_name: String,
}


#[derive(Debug, Clone, RustcEncodable, Queryable)]
pub struct PhotoTag {
    pub id: i32,
    pub photo_id: i32,
    pub tag_id: i32,
}

use super::schema::tags;
#[insertable_into(tags)]
#[derive(Debug, Clone)]
pub struct NewTag<'a> {
    pub tag_name: &'a str,
    pub slug: &'a str,
}

use super::schema::photo_tags;
#[insertable_into(photo_tags)]
#[derive(Debug, Clone)]
pub struct NewPhotoTag {
    pub photo_id: i32,
    pub tag_id: i32,
}


/*
impl Entity for Tag {
    fn id(&self) -> &ToValue {
        &self.id
    }
}
impl IsDao for Tag {
    fn from_dao(dao: &Dao) -> Self {
        Tag {
            id: dao.get("id"),
            tag: dao.get("tag"),
            slug: dao.get("slug"),
        }
    }
    fn to_dao(&self) -> Dao {
        let mut dao = Dao::new();
        dao.set("id", &self.id);
        dao.set("tag", &self.tag);
        dao.set("slug", &self.slug);
        dao
    }
}

impl IsTable for Tag {
    fn table() -> Table {
        table("tag",
              vec![Column {
                       name: "id".to_string(),
                       data_type: Type::I32,
                       db_data_type: "serial".to_string(),
                       is_primary: true,
                       is_unique: true,
                       default: None,
                       comment: None,
                       not_null: true,
                       foreign: None,
                       is_inherited: false,
                   },
                   Column {
                       name: "tag".to_string(),
                       data_type: Type::String,
                       db_data_type: "varchar(100)".to_string(),
                       is_primary: false,
                       is_unique: true,
                       default: None,
                       comment: None,
                       not_null: true,
                       foreign: None,
                       is_inherited: false,
                   },
                   Column {
                       name: "slug".to_string(),
                       data_type: Type::String,
                       db_data_type: "varchar(100)".to_string(),
                       is_primary: false,
                       is_unique: true,
                       default: None,
                       comment: None,
                       not_null: true,
                       foreign: None,
                       is_inherited: false,
                   }])
    }
}
*/
#[derive(Debug, Clone, RustcEncodable, Queryable)]
pub struct Person {
    pub id: i32,
    pub slug: String,
    pub person_name: String,
}

#[derive(Debug, Clone, RustcEncodable, Queryable)]
pub struct PhotoPerson {
    pub id: i32,
    pub photo_id: i32,
    pub person_id: i32,
}

use super::schema::people;
#[insertable_into(people)]
#[derive(Debug, Clone)]
pub struct NewPerson<'a> {
    pub person_name: &'a str,
    pub slug: &'a str,
}

use super::schema::photo_people;
#[insertable_into(photo_people)]
#[derive(Debug, Clone)]
pub struct NewPhotoPerson {
    pub photo_id: i32,
    pub person_id: i32,
}

/*
impl Entity for Person {
    fn id(&self) -> &ToValue {
        &self.id
    }
}
impl IsDao for Person {
    fn from_dao(dao: &Dao) -> Self {
        Person {
            id: dao.get("id"),
            name: dao.get("name"),
            slug: dao.get("slug"),
        }
    }
    fn to_dao(&self) -> Dao {
        let mut dao = Dao::new();
        dao.set("id", &self.id);
        dao.set("name", &self.name);
        dao.set("slug", &self.slug);
        dao
    }
}

impl IsTable for Person {
    fn table() -> Table {
        table("person",
              vec![Column {
                       name: "id".to_string(),
                       data_type: Type::I32,
                       db_data_type: "serial".to_string(),
                       is_primary: true,
                       is_unique: true,
                       default: None,
                       comment: None,
                       not_null: true,
                       foreign: None,
                       is_inherited: false,
                   },
                   Column {
                       name: "name".to_string(),
                       data_type: Type::String,
                       db_data_type: "varchar(100)".to_string(),
                       is_primary: false,
                       is_unique: true,
                       default: None,
                       comment: None,
                       not_null: true,
                       foreign: None,
                       is_inherited: false,
                   },
                   Column {
                       name: "slug".to_string(),
                       data_type: Type::String,
                       db_data_type: "varchar(100)".to_string(),
                       is_primary: false,
                       is_unique: true,
                       default: None,
                       comment: None,
                       not_null: true,
                       foreign: None,
                       is_inherited: false,
                   }])
    }
}
*/
#[derive(Debug, Clone, RustcEncodable, Queryable)]
pub struct Place {
    pub id: i32,
    pub slug: String,
    pub place_name: String,
}

#[derive(Debug, Clone, RustcEncodable, Queryable)]
pub struct PhotoPlace {
    pub id: i32,
    pub photo_id: i32,
    pub place_id: i32,
}

use super::schema::places;
#[insertable_into(places)]
#[derive(Debug, Clone)]
pub struct NewPlace<'a> {
    pub slug: &'a str,
    pub place_name: &'a str,
}

use super::schema::photo_places;
#[insertable_into(photo_places)]
#[derive(Debug, Clone)]
pub struct NewPhotoPlace {
    pub photo_id: i32,
    pub place_id: i32,
}

/*
impl Entity for Place {
    fn id(&self) -> &ToValue {
        &self.id
    }
}
impl IsDao for Place {
    fn from_dao(dao: &Dao) -> Self {
        Place {
            id: dao.get("id"),
            place: dao.get("place"),
            slug: dao.get("slug"),
        }
    }
    fn to_dao(&self) -> Dao {
        let mut dao = Dao::new();
        dao.set("id", &self.id);
        dao.set("place", &self.place);
        dao.set("slug", &self.slug);
        dao
    }
}

impl IsTable for Place {
    fn table() -> Table {
        table("place",
              vec![Column {
                       name: "id".to_string(),
                       data_type: Type::I32,
                       db_data_type: "serial".to_string(),
                       is_primary: true,
                       is_unique: true,
                       default: None,
                       comment: None,
                       not_null: true,
                       foreign: None,
                       is_inherited: false,
                   },
                   Column {
                       name: "place".to_string(),
                       data_type: Type::String,
                       db_data_type: "varchar(100)".to_string(),
                       is_primary: false,
                       is_unique: true,
                       default: None,
                       comment: None,
                       not_null: true,
                       foreign: None,
                       is_inherited: false,
                   },
                   Column {
                       name: "slug".to_string(),
                       data_type: Type::String,
                       db_data_type: "varchar(100)".to_string(),
                       is_primary: false,
                       is_unique: true,
                       default: None,
                       comment: None,
                       not_null: true,
                       foreign: None,
                       is_inherited: false,
                   }])
    }
}

fn table(name: &str, columns: Vec<Column>) -> Table {
    Table {
        schema: None,
        name: name.to_owned(),
        parent_table: None,
        sub_table: vec![],
        comment: None,
        columns: columns,
        is_view: false,
    }
}


pub fn query_for<T: IsTable>() -> Query {
    let mut q = Query::select();
    q.from(&T::table());
    q
}

#[allow(dead_code)]
pub fn get_or_create<'a, T: IsTable + IsDao>(db: &Database,
                                             key: &str,
                                             val: &ToValue,
                                             defaults: &[(&str, &ToValue)])
                                             -> T {
    if let Ok(result) = query_for::<T>().filter_eq(key, val).collect_one(db) {
        result
    } else {
        let table = T::table();
        let mut q = Query::insert();
        q.into_(&table);
        q.set(key, val);
        for p in defaults {
            let &(key, f) = p;
            q.set(key, f);
        }
        q.returns(table.columns.iter().map(|c| &*c.name).collect())
         .collect_one(db)
         .unwrap()
    }
}
*/
