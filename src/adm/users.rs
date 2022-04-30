use super::result::Error;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{insert_into, update};
use djangohashers::make_password;
use rand::{thread_rng, Rng};
use std::iter::Iterator;

pub fn list(db: &mut PgConnection) -> Result<(), Error> {
    use crate::schema::users::dsl::*;
    println!(
        "Existing users: {:?}.",
        users.select(username).load::<String>(db)?,
    );
    Ok(())
}

pub fn passwd(db: &mut PgConnection, uname: &str) -> Result<(), Error> {
    let pword = random_password(14);
    let hashword = make_password(&pword);
    use crate::schema::users::dsl::*;
    match update(users.filter(username.eq(&uname)))
        .set(password.eq(&hashword))
        .execute(db)?
    {
        1 => {
            println!("Updated password for {:?} to {:?}", uname, pword);
        }
        0 => {
            insert_into(users)
                .values((username.eq(uname), password.eq(&hashword)))
                .execute(db)?;
            println!("Created user {:?} with password {:?}", uname, pword);
        }
        n => {
            println!(
                "Strange, updated {} passwords for {:?} to {:?}",
                n, uname, pword,
            );
        }
    };
    Ok(())
}

fn random_password(len: usize) -> String {
    let rng = thread_rng();
    // Note; I would like to have lowercase letters more probable
    use rand::distributions::Alphanumeric;
    rng.sample_iter(&Alphanumeric)
        .map(char::from)
        .take(len)
        .collect()
}
