//! Just fooling around with different ways to count images per year.
extern crate rphotos;
extern crate brotli2;
extern crate clap;
extern crate chrono;
#[macro_use]
extern crate diesel;
extern crate djangohashers;
extern crate dotenv;
extern crate env_logger;
extern crate flate2;
extern crate rand;
extern crate rexif;
#[macro_use]
extern crate log;
extern crate image;
extern crate xml;

mod adm;
mod env;
mod photosdir;

use adm::{findphotos, makepublic, readkpa, users, storestatics};
use adm::result::Error;
use adm::stats::show_stats;
use clap::{App, Arg, ArgMatches, SubCommand};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use env::{dburl, photos_dir};
use photosdir::PhotosDir;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;
use std::process::exit;

fn main() {
    dotenv().ok();
    env_logger::init().unwrap();
    let args = App::new("rphotoadm")
        .about("Command line interface for rphotos")
        .subcommand(SubCommand::with_name("findphotos")
            .about("Find new photos in the photo directory")
            .arg(Arg::with_name("BASE")
                .multiple(true)
                .help("Base directory to search in (relative to the \
                       image root).")))
        .subcommand(SubCommand::with_name("readkpa")
            .about("Read metadata from my kphotoalbum"))
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
        .subcommand(SubCommand::with_name("storestatics")
            .about("Store statics as files for a web server")
            .arg(Arg::with_name("DIR")
                .required(true)
                .help("Directory to store the files in")))
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
        ("findphotos", Some(args)) => {
            let pd = PhotosDir::new(photos_dir());
            let db = try!(get_db());
            if let Some(bases) = args.values_of("BASE") {
                for base in bases {
                    try!(findphotos::crawl(&db, &pd, Path::new(&base))
                         .map_err(|e| Error::Other(
                             format!("Failed to crawl {}: {}", base, e))));
                }
            } else {
                try!(findphotos::crawl(&db, &pd, Path::new(""))
                     .map_err(|e| Error::Other(
                         format!("Failed to crawl: {}", e))));
            }
            Ok(())
        }
        ("makepublic", Some(args)) => {
            let pd = PhotosDir::new(photos_dir());
            let db = try!(get_db());
            match args.value_of("LIST") {
                Some("-") => {
                    let list = io::stdin();
                    try!(makepublic::by_file_list(&db, &pd, list.lock()));
                    Ok(())
                }
                Some(f) => {
                    let list = try!(File::open(f));
                    let list = BufReader::new(list);
                    makepublic::by_file_list(&db, &pd, list)
                }
                None => {
                    makepublic::one(&db, &pd, args.value_of("IMAGE").unwrap())
                }
            }
        }
        ("readkpa", Some(_args)) => {
            readkpa::readkpa(&try!(get_db()), &photos_dir())
        }
        ("stats", Some(_args)) => show_stats(&try!(get_db())),
        ("userlist", Some(_args)) => users::list(&try!(get_db())),
        ("userpass", Some(args)) => {
            users::passwd(&try!(get_db()), args.value_of("USER").unwrap())
        }
        ("storestatics", Some(args)) => {
            storestatics::to_dir(args.value_of("DIR").unwrap())
        }
        _ => Ok(println!("No subcommand given.\n\n{}", args.usage())),
    }
}

fn get_db() -> Result<PgConnection, ConnectionError> {
    PgConnection::establish(&dburl())
}
