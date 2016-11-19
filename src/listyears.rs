//! Just fooling around with different ways to count images per year.
extern crate rphotos;
#[macro_use]
extern crate diesel;
extern crate dotenv;
extern crate env_logger;

mod env;

use dotenv::dotenv;
use diesel::expression::dsl::{count_star, sql};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::types::{BigInt, Double, Nullable, Text, Timestamp};
use env::dburl;


sql_function!(date_part,
              date_part_t,
              (part: Text, date: Nullable<Timestamp>) -> Nullable<Double>);

fn main() {
    dotenv().ok();
    env_logger::init().unwrap();
    let db = PgConnection::establish(&dburl())
                 .expect("Error connecting to database");
    use rphotos::schema::photos::dsl::{date, photos};

    println!("Nice semantics, but no group: {:?}",
             photos.select(date_part("year", date))
                   .limit(10)
                   .load::<Option<f64>>(&db));
    println!("Groups: {:?}",
             photos.select(sql::<Nullable<Double>>("extract(year from date) y"))
                   .group_by(sql::<Nullable<Double>>("y"))
                   .order(sql::<Nullable<Double>>("y"))
                   .load::<Option<f64>>(&db));

    println!("Count all: {:?}",
             photos.select(count_star()).load::<i64>(&db));

    println!("Count per year: {:?}",
             photos.select(sql::<(Nullable<Double>, BigInt)>("extract(year \
                                                              from date) y, \
                                                              count(*)"))
                   .group_by(sql::<Nullable<Double>>("y"))
                   .order(sql::<Nullable<Double>>("y"))
                   .load::<(Option<f64>, i64)>(&db));

    /*
    println!("{:?}",
             photos.select((sql::<Nullable<Double>>("extract(year from date) \
                                                     y"),
                            count_star()))
                   .group_by(sql::<Nullable<Double>>("y"))
                   .order(sql::<Nullable<Double>>("y"))
                   .load::<(Option<f64>, i64)>(&db));
    */
}
