use super::result::Error;
use crate::schema::users::dsl as u;
use diesel::prelude::*;
use diesel::{insert_into, update};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use djangohashers::make_password;
use rand::{Rng, distr, rng};
use std::iter::Iterator;

pub async fn list(db: &mut AsyncPgConnection) -> Result<(), Error> {
    println!(
        "Existing users: {:?}.",
        u::users.select(u::username).load::<String>(db).await?,
    );
    Ok(())
}

pub async fn passwd(
    db: &mut AsyncPgConnection,
    uname: &str,
) -> Result<(), Error> {
    let pword = random_password(14);
    let hashword = make_password(&pword);
    match update(u::users.filter(u::username.eq(&uname)))
        .set(u::password.eq(&hashword))
        .execute(db)
        .await?
    {
        1 => {
            println!("Updated password for {uname:?} to {pword:?}");
        }
        0 => {
            insert_into(u::users)
                .values((u::username.eq(uname), u::password.eq(&hashword)))
                .execute(db)
                .await?;
            println!("Created user {uname:?} with password {pword:?}");
        }
        n => {
            println!(
                "Strange, updated {n} passwords for {uname:?} to {pword:?}",
            );
        }
    };
    Ok(())
}

fn random_password(len: usize) -> String {
    // Note; I would like to have lowercase letters more probable
    rng()
        .sample_iter(&distr::Alphanumeric)
        .map(char::from)
        .take(len)
        .collect()
}
