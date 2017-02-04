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
    use rphotos::schema::users::dsl::*;
    println!("Existing users: {:?}.",
             try!(users.select(username).load::<String>(db)));
    Ok(())
}

pub fn passwd(db: &PgConnection, uname: &str) -> Result<(), Error> {
    let pword = random_password(14);
    let hashword = make_password(&pword);
    use rphotos::schema::users::dsl::*;
    match try!(update(users.filter(username.eq(&uname)))
        .set(password.eq(&hashword))
        .execute(db)) {
        1 => {
            println!("Updated password for {:?} to {:?}", uname, pword);
        }
        0 => {
            use rphotos::models::NewUser;
            try!(insert(&NewUser {
                    username: &uname,
                    password: &hashword,
                })
                .into(users)
                .execute(db));
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
    let nlc = 'z' as u8 - 'a' as u8 + 1;
    let x = Range::new(0, 6 * nlc + 4 * 10 + 1);
    (0..len)
        .map(|_| match x.ind_sample(rng) {
            n if n < (1 * nlc) => ('A' as u8 + (n % nlc)) as char,
            n if n < (6 * nlc) => ('a' as u8 + (n % nlc)) as char,
            n => ('0' as u8 + n % 10) as char,
        })
        .collect()
}
