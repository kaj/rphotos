//! Just fooling around with different ways to count images per year.
extern crate rphotos;
extern crate clap;
#[macro_use]
extern crate diesel;
extern crate djangohashers;
extern crate dotenv;
extern crate env_logger;
extern crate rand;

mod adm;
mod env;

use adm::result::Error;
use adm::stats::show_stats;
use adm::users;
use clap::{App, Arg, ArgMatches, SubCommand};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use env::dburl;
use std::process::exit;

fn main() {
    dotenv().ok();
    env_logger::init().unwrap();
    let args = App::new("rphotoadm")
        .about("Command line interface for rphotos")
        .subcommand(SubCommand::with_name("stats")
            .about("Show some statistics from the database"))
        .subcommand(SubCommand::with_name("userlist")
            .about("List users"))
        .subcommand(SubCommand::with_name("userpass")
            .about("Set password for a (new or existing) user")
             .arg(Arg::with_name("USER")
                  .required(true)
                  .help("Username to set password for")))
        .get_matches();

    match run(args) {
        Ok(()) => (),
        Err(err) => {
            println!("{}", err);
            exit(1);
        }
    }
}

fn run(args: ArgMatches) -> Result<(), Error> {
    match args.subcommand() {
        ("stats", Some(_args)) => show_stats(&try!(get_db())),
        ("userlist", Some(_args)) => users::list(&try!(get_db())),
        ("userpass", Some(args)) => users::passwd(&try!(get_db()),
                                                   args.value_of("USER").unwrap()),
        _ => Ok(println!("No subcommand given.\n\n{}", args.usage())),
    }
}

fn get_db() -> Result<PgConnection, ConnectionError> {
    PgConnection::establish(&dburl())
}
