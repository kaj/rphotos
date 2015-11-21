//extern crate chrono;

use rustorm::query::Query;
use rustorm::dao::{Dao,IsDao};
use rustorm::table::{IsTable, Table};

#[derive(Debug, Clone, RustcEncodable)]
pub struct Photo {
    pub id: i32,
    pub path: String,
}

impl IsDao for Photo {
    fn from_dao(dao:&Dao) -> Self {
        Photo {
            id: dao.get("id"),
            path: dao.get("path"),
        }
    }
    fn to_dao(&self) -> Dao {
        let mut dao = Dao::new();
        dao.set("id", &self.id);
        dao.set("path", &self.path);
        dao
    }
}

impl IsTable for Photo {
    fn table() -> Table {
        Table {
            schema: "public".to_owned(),
            name: "photo".to_owned(),
            parent_table: None,
            sub_table: vec![],
            comment: None,
            columns: vec![],
            is_view: false
        }
    }
}

pub fn query_for<T: IsTable>() -> Query {
    let mut q = Query::select_all();
    q.from_table(&T::table().complete_name());
    q
}

