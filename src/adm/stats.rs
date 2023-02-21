use super::result::Error;
use crate::schema::people::dsl::people;
use crate::schema::photos::dsl::{self as p, photos};
use crate::schema::places::dsl::places;
use crate::schema::tags::dsl::tags;
use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::sql_types::{Nullable, Timestamp};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

sql_function! {
    #[aggregate]
    fn year_of_timestamp(date: Nullable<Timestamp>) -> Nullable<SmallInt>
}

pub async fn show_stats(db: &mut AsyncPgConnection) -> Result<(), Error> {
    println!(
        "There are {} photos in total.",
        photos.select(count_star()).first::<i64>(db).await?,
    );

    println!(
        "There are {} persons, {} places, and {} tags mentioned.",
        people.select(count_star()).first::<i64>(db).await?,
        places.select(count_star()).first::<i64>(db).await?,
        tags.select(count_star()).first::<i64>(db).await?,
    );

    let y = year_of_timestamp(p::date);
    println!(
        "Count per year: {:?}",
        photos
            .select((y, count_star()))
            .group_by(y)
            .order(y.desc().nulls_last())
            .load::<(Option<i16>, i64)>(db)
            .await?
            .iter()
            .map(|&(y, n)| format!("{}: {}", y.unwrap_or(0), n))
            .collect::<Vec<_>>(),
    );

    Ok(())
}
