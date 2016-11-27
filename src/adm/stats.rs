#![allow(dead_code)] // for the date_part macro-created function
use adm::result::Error;
use diesel::expression::dsl::{count_star, sql};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::types::{BigInt, Double, Nullable, Text, Timestamp};
use rphotos::schema::people::dsl::people;
use rphotos::schema::photos::dsl::photos;
use rphotos::schema::places::dsl::places;
use rphotos::schema::tags::dsl::tags;

sql_function!(date_part,
              date_part_t,
              (part: Text, date: Nullable<Timestamp>) -> Nullable<Double>);

pub fn show_stats(db: &PgConnection) -> Result<(), Error> {

    println!("There are {} photos in total.",
             try!(photos.select(count_star()).first::<i64>(db)));

    println!("There are {} persons, {} places, and {} tags mentioned.",
             try!(people.select(count_star()).first::<i64>(db)),
             try!(places.select(count_star()).first::<i64>(db)),
             try!(tags.select(count_star()).first::<i64>(db)));

    // Something like this should be possible, I guess?
    //
    // use rphotos::schema::photos::dsl::date;
    // let year = date_part("year", date).aliased("y");
    // println!("Count per year: {:?}",
    //          photos.select((year, count_star()))
    //              .group_by(year)
    //              .limit(10)
    //              .load::<(Option<f64>, i64)>(db));

    println!("Count per year: {:?}",
             try!(photos.select(sql::<(Nullable<Double>, BigInt)>(
                 "extract(year from date) y, count(*)"))
                  .group_by(sql::<Nullable<Double>>("y"))
                  .order(sql::<Nullable<Double>>("y").desc().nulls_last())
                  .load::<(Option<f64>, i64)>(db))
                 .iter()
                 .map(|&(y, n)| format!("{}: {}", y.unwrap_or(0.0), n))
                 .collect::<Vec<_>>());

    Ok(())
}
