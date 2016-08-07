#[macro_use]
extern crate log;
extern crate env_logger;
extern crate dotenv;
extern crate diesel;
extern crate rphotos;

use dotenv::dotenv;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use rphotos::models::Photo;
use std::io;
use std::io::prelude::*;

mod env;
use env::dburl;


fn main() {
    dotenv().ok();
    env_logger::init().unwrap();
    let db = PgConnection::establish(&dburl())
                 .expect("Error connecting to database");

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(line) => {
                use rphotos::schema::photos::dsl::*;
                info!("Shold make {} public: {:?}",
                      line,
                      diesel::update(photos.filter(path.eq(&line)))
                          .set(is_public.eq(true))
                          .get_result::<Photo>(&db));
            }
            Err(err) => {
                println!("Failed to read a line: {:?}", err);
            }
        }
    }
}
