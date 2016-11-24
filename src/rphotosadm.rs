//! Just fooling around with different ways to count images per year.
extern crate rphotos;
extern crate clap;
#[macro_use]
extern crate diesel;
extern crate djangohashers;
extern crate dotenv;
extern crate env_logger;
extern crate rand;
#[macro_use]
extern crate log;
extern crate image;
extern crate rexif;

mod adm;
mod env;
mod photosdir;

use adm::result::Error;
use adm::stats::show_stats;
use adm::{makepublic, users};
use clap::{App, Arg, ArgMatches, SubCommand};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use env::{dburl, photos_dir};
use photosdir::PhotosDir;
use std::fs::File;
use std::io::{self, BufReader};
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
        .subcommand(SubCommand::with_name("makepublic")
            .about("make specific image(s) public")
            .arg(Arg::with_name("LIST")
                 .long("list")
                 .short("l")
                 .takes_value(true)
                 .help("File listing image paths to make public"))
            .arg(Arg::with_name("IMAGE")
                 .required_unless("LIST")
                 .help("Image path to make public"))
            .after_help("The image path(s) are relative to the \
                         image root."))
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
        ("makepublic", Some(args)) => {
            let pd = PhotosDir::new(photos_dir());
            if let Some(f) = args.value_of("LIST") {
                if f == "-" {
                    let list = io::stdin();
                    try!(makepublic::by_file_list(&try!(get_db()),
                                                  &pd,
                                                  list.lock()));
                } else {
                    let list = try!(File::open(f));
                    let list = BufReader::new(list);
                    try!(makepublic::by_file_list(&try!(get_db()),
                                                  &pd,
                                                  list));
                }
                Ok(())
            } else {
                makepublic::one(&try!(get_db()),
                                &pd,
                                args.value_of("IMAGE").unwrap())
            }
        }
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
