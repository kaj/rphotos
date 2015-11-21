#[macro_use] extern crate nickel;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate rustorm;
extern crate rustc_serialize;

use std::collections::HashMap;
use nickel::Nickel;
use rustorm::pool::ManagedPool;
use std::env::var;
use std::process::exit;

mod models;
use models::{Photo, query_for};

fn dburl() -> String {
    let db_var = "RPHOTOS_DB";
    match var(db_var) {
        Ok(result) => result,
        Err(error) => {
            println!("A database url needs to be given in env {}: {}",
                     db_var, error);
            exit(1);
        }
    }
}


fn main() {
    env_logger::init().unwrap();
    info!("Initalized logger");
    // NOTE pool will need to be mut if we do db writes?
    let pool = ManagedPool::init(&dburl(), 1).unwrap();
    info!("Initalized pool");

    let mut server = Nickel::new();

    server.utilize(router! {
        get "/" => |_req, res| {
            //let db = pool.connect().unwrap();
            let photos: Vec<Photo> = query_for::<Photo>()
                .collect(pool.connect().unwrap().as_ref()).unwrap();
            info!("Got some photos: {:?}", photos);
            let mut data = HashMap::new();
            data.insert("photos", &photos);
            // data.insert("name", "Self".into());
            return res.render("templates/index.tpl", &data);
        }
    });

    server.listen("127.0.0.1:6767");
}
