use adm::result::Error;
use diesel::{insert, update};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use djangohashers::make_password;
use rand::distributions::IndependentSample;
use rand::distributions::range::Range;
use rand::os::OsRng;
use std::iter::Iterator;

pub fn list(db: &PgConnection) -> Result<(), Error> {
    use schema::users::dsl::*;
    println!("Existing users: {:?}.",
             users.select(username).load::<String>(db)?);
    Ok(())
}

pub fn passwd(db: &PgConnection, uname: &str) -> Result<(), Error> {
    let pword = random_password(14);
    let hashword = make_password(&pword);
    use schema::users::dsl::*;
    match update(users.filter(username.eq(&uname)))
        .set(password.eq(&hashword))
        .execute(db)? {
        1 => {
            println!("Updated password for {:?} to {:?}", uname, pword);
        }
        0 => {
            use models::NewUser;
            insert(&NewUser {
                    username: uname,
                    password: &hashword,
                })
                .into(users)
                .execute(db)?;
            println!("Created user {:?} with password {:?}", uname, pword);
        }
        n => {
            println!("Strange, updated {} passwords for {:?} to {:?}",
                     n,
                     uname,
                     pword);
        }
    };
    Ok(())
}

fn random_password(len: usize) -> String {
    let rng = &mut OsRng::new().expect("Init rng");
    let nlc = b'z' - b'a' + 1;
    let x = Range::new(0, 6 * nlc + 4 * 10 + 1);
    (0..len)
        .map(|_| match x.ind_sample(rng) {
            n if n < (1 * nlc) => (b'A' + (n % nlc)) as char,
            n if n < (6 * nlc) => (b'a' + (n % nlc)) as char,
            n => (b'0' + n % 10) as char,
        })
        .collect()
}
