extern crate dotenv;
extern crate env_logger;
extern crate djangohashers;
extern crate rand;
extern crate rphotos;
extern crate diesel;

use dotenv::dotenv;
use std::iter::Iterator;
use rand::os::OsRng;
use rand::distributions::range::Range;
use rand::distributions::IndependentSample;
use djangohashers::make_password;
use diesel::prelude::*;
mod env;
use env::dburl;
use diesel::pg::PgConnection;

fn main() {
    dotenv().ok();
    env_logger::init().unwrap();
    let db = PgConnection::establish(&dburl())
                 .expect("Error connecting to database");
    let uname = std::env::args().nth(1).expect("username argument missing");
    let pword = random_password(14);
    println!("User {} has password {}", uname, pword);
    let hashword = make_password(&pword);
    use rphotos::schema::users::dsl::*;
    match diesel::update(users.filter(username.eq(&uname)))
              .set(password.eq(&hashword))
              .execute(&db) {
        Ok(1) => {
            println!("Updated password");
        }
        Ok(0) => {
            use rphotos::models::NewUser;
            diesel::insert(&NewUser {
                username: &uname,
                password: &hashword,
            })
                .into(users)
                .execute(&db)
                .expect("Create user");
            println!("New user");
        }
        Ok(n) => {
            println!("Strange, update {} users", n);
        }
        Err(err) => {
            println!("Error: {}", err);
        }
    };
}

fn random_password(len: usize) -> String {
    let rng = &mut OsRng::new().expect("Init rng");
    let nlc = 'z' as u8 - 'a' as u8 + 1;
    let x = Range::new(0, 6 * nlc + 2 * 10 + 1);
    (0..len)
        .map(|_| {
            match x.ind_sample(rng) {
                n if n < (1 * nlc) => ('A' as u8 + (n % nlc)) as char,
                n if n < (6 * nlc) => ('a' as u8 + (n % nlc)) as char,
                n => ('0' as u8 + (n - 4 * nlc) % 10) as char,
            }
        })
        .collect()
}
