#![recursion_limit = "128"]
extern crate brotli2;
extern crate chrono;
extern crate clap;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_infer_schema;
extern crate djangohashers;
extern crate dotenv;
extern crate env_logger;
extern crate flate2;
extern crate hyper;
extern crate image;
extern crate libc;
#[macro_use]
extern crate log;
extern crate memcached;
#[macro_use]
extern crate nickel;
extern crate nickel_jwt_session;
extern crate plugin;
extern crate r2d2;
extern crate r2d2_diesel;
extern crate rand;
extern crate regex;
extern crate rexif;
extern crate rustc_serialize;
extern crate slug;
extern crate time;
extern crate typemap;

mod adm;
mod env;
mod memcachemiddleware;
mod models;
mod nickel_diesel;
mod photosdir;
mod photosdirmiddleware;
mod pidfiles;
mod requestloggermiddleware;
mod schema;
mod server;

use adm::{findphotos, makepublic, precache, storestatics, users};
use adm::result::Error;
use adm::stats::show_stats;
use clap::{App, Arg, ArgMatches, SubCommand};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use env::{dburl, photos_dir};
use photosdir::PhotosDir;
pub use server::Link;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;
use std::process::exit;

fn main() {
    dotenv().ok();
    env_logger::init();
    let args = App::new("rphotos")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Command line interface for rphotos")
        .subcommand(
            SubCommand::with_name("findphotos")
                .about("Find new photos in the photo directory")
                .arg(Arg::with_name("BASE").multiple(true).help(
                    "Base directory to search in (relative to the \
                     image root).",
                )),
        )
        .subcommand(
            SubCommand::with_name("stats")
                .about("Show some statistics from the database"),
        )
        .subcommand(
            SubCommand::with_name("userlist").about("List existing users"),
        )
        .subcommand(
            SubCommand::with_name("userpass")
                .about("Set password for a (new or existing) user")
                .arg(
                    Arg::with_name("USER")
                        .required(true)
                        .help("Username to set password for"),
                ),
        )
        .subcommand(
            SubCommand::with_name("makepublic")
                .about("make specific image(s) public")
                .arg(
                    Arg::with_name("LIST")
                        .long("list")
                        .short("l")
                        .takes_value(true)
                        .help("File listing image paths to make public"),
                )
                .arg(
                    Arg::with_name("IMAGE")
                        .required_unless("LIST")
                        .help("Image path to make public"),
                )
                .after_help(
                    "The image path(s) are relative to the image root.",
                ),
        )
        .subcommand(
            SubCommand::with_name("precache")
                .about("Make sure the photos has thumbnails stored in cache."),
        )
        .subcommand(
            SubCommand::with_name("storestatics")
                .about("Store statics as files for a web server")
                .arg(
                    Arg::with_name("DIR")
                        .required(true)
                        .help("Directory to store the files in"),
                ),
        )
        .subcommand(
            SubCommand::with_name("runserver")
                .arg(
                    Arg::with_name("PIDFILE")
                        .long("pidfile")
                        .takes_value(true)
                        .help(
                            "Write (and read, if --replace) a pid file with \
                             the name given as <PIDFILE>.",
                        ),
                )
                .arg(Arg::with_name("REPLACE").long("replace").help(
                    "Kill old server (identified by pid file) before running",
                )),
        )
        .get_matches();

    match run(&args) {
        Ok(()) => (),
        Err(err) => {
            println!("{}", err);
            exit(1);
        }
    }
}

fn run(args: &ArgMatches) -> Result<(), Error> {
    match args.subcommand() {
        ("findphotos", Some(args)) => {
            let pd = PhotosDir::new(photos_dir());
            let db = get_db()?;
            if let Some(bases) = args.values_of("BASE") {
                for base in bases {
                    findphotos::crawl(&db, &pd, Path::new(&base)).map_err(
                        |e| {
                            Error::Other(format!(
                                "Failed to crawl {}: {}",
                                base, e,
                            ))
                        },
                    )?;
                }
            } else {
                findphotos::crawl(&db, &pd, Path::new("")).map_err(|e| {
                    Error::Other(format!("Failed to crawl: {}", e))
                })?;
            }
            Ok(())
        }
        ("makepublic", Some(args)) => {
            let pd = PhotosDir::new(photos_dir());
            let db = get_db()?;
            match args.value_of("LIST") {
                Some("-") => {
                    let list = io::stdin();
                    makepublic::by_file_list(&db, &pd, list.lock())?;
                    Ok(())
                }
                Some(f) => {
                    let list = File::open(f)?;
                    let list = BufReader::new(list);
                    makepublic::by_file_list(&db, &pd, list)
                }
                None => {
                    makepublic::one(&db, &pd, args.value_of("IMAGE").unwrap())
                }
            }
        }
        ("stats", Some(_args)) => show_stats(&get_db()?),
        ("userlist", Some(_args)) => users::list(&get_db()?),
        ("userpass", Some(args)) => {
            users::passwd(&get_db()?, args.value_of("USER").unwrap())
        }
        ("precache", _) => {
            precache::precache(&get_db()?, &PhotosDir::new(photos_dir()))
        }
        ("storestatics", Some(args)) => {
            storestatics::to_dir(args.value_of("DIR").unwrap())
        }
        ("runserver", Some(args)) => server::run(args),
        _ => Ok(println!("No subcommand given.\n\n{}", args.usage())),
    }
}

fn get_db() -> Result<PgConnection, ConnectionError> {
    PgConnection::establish(&dburl())
}

include!(concat!(env!("OUT_DIR"), "/templates.rs"));
