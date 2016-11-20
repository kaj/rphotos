//! Just fooling around with different ways to count images per year.
extern crate rphotos;
extern crate clap;
#[macro_use]
extern crate diesel;
extern crate dotenv;
extern crate env_logger;

mod adm;
mod env;

use adm::stats::show_stats;
use clap::{App, ArgMatches, SubCommand};
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
        .get_matches();

    match run(args) {
        Ok(()) => (),
        Err(err) => {
            println!("{}", err);
            exit(1);
        }
    }
}

fn run(args: ArgMatches) -> Result<(), ConnectionError> {
    match args.subcommand() {
        ("stats", Some(_args)) => show_stats(&try!(get_db())),
        _ => Ok(println!("No subcommand given.\n\n{}", args.usage())),
    }
}

fn get_db() -> Result<PgConnection, ConnectionError> {
    PgConnection::establish(&dburl())
}
