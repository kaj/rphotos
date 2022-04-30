#![allow(proc_macro_derive_resolution_fallback)]
#![recursion_limit = "128"]
#[macro_use]
extern crate diesel;

mod adm;
mod dbopt;
mod fetch_places;
mod models;
mod myexif;
mod photosdir;
mod pidfiles;
mod schema;
mod server;

use crate::adm::result::Error;
use crate::adm::stats::show_stats;
use crate::adm::{findphotos, makepublic, precache, storestatics, users};
use crate::dbopt::DbOpt;
use clap::Parser;
use dotenv::dotenv;
use std::path::PathBuf;
use std::process::exit;

/// Command line interface for rphotos.
#[derive(Parser)]
enum RPhotos {
    /// Make specific image(s) public.
    ///
    /// The image path(s) are relative to the image root.
    Makepublic(makepublic::Makepublic),
    /// Get place tags for photos by looking up coordinates in OSM
    Fetchplaces(fetch_places::Fetchplaces),
    /// Find new photos in the photo directory
    Findphotos(findphotos::Findphotos),
    /// Make sure the photos has thumbnails stored in cache.
    ///
    /// The time limit is checked after each stored image, so the
    /// command will complete in slightly more than the max time and
    /// one image will be processed even if the max time is zero.
    Precache(precache::Args),
    /// Show some statistics from the database
    Stats(DbOpt),
    /// Store statics as files for a web server
    Storestatics {
        /// Directory to store the files in
        dir: String,
    },
    /// List existing users
    Userlist {
        #[clap(flatten)]
        db: DbOpt,
    },
    /// Set password for a (new or existing) user
    Userpass {
        #[clap(flatten)]
        db: DbOpt,
        /// Username to set password for
        // TODO: Use a special type that only accepts nice user names.
        user: String,
    },
    /// Run the rphotos web server.
    Runserver(server::Args),
}

#[derive(clap::Parser)]
struct CacheOpt {
    /// How to connect to memcached.
    #[clap(
        long,
        env = "MEMCACHED_SERVER",
        default_value = "memcache://127.0.0.1:11211"
    )]
    memcached_url: String,
}

#[derive(clap::Parser)]
struct DirOpt {
    /// Path to the root directory storing all actual photos.
    #[clap(long, env = "RPHOTOS_DIR")]
    photos_dir: PathBuf,
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").as_deref().unwrap_or("info"),
        )
        .init();
    match run(&RPhotos::from_args()).await {
        Ok(()) => (),
        Err(err) => {
            println!("{}", err);
            exit(1);
        }
    }
}

async fn run(args: &RPhotos) -> Result<(), Error> {
    match args {
        RPhotos::Findphotos(cmd) => cmd.run(),
        RPhotos::Makepublic(cmd) => cmd.run(),
        RPhotos::Stats(db) => show_stats(&mut db.connect()?),
        RPhotos::Userlist { db } => users::list(&mut db.connect()?),
        RPhotos::Userpass { db, user } => {
            users::passwd(&mut db.connect()?, user)
        }
        RPhotos::Fetchplaces(cmd) => cmd.run().await,
        RPhotos::Precache(cmd) => cmd.run().await,
        RPhotos::Storestatics { dir } => storestatics::to_dir(dir),
        RPhotos::Runserver(ra) => server::run(ra).await,
    }
}

include!(concat!(env!("OUT_DIR"), "/templates.rs"));
