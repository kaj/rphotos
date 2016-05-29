#![cfg_attr(feature = "nightly", feature(custom_derive, custom_attribute, plugin))]
#![cfg_attr(feature = "nightly", plugin(diesel_codegen, dotenv_macros))]

#[macro_use]
extern crate diesel;
extern crate dotenv;

#[cfg(feature = "nightly")]
include!("lib.in.rs");

#[cfg(feature = "with-syntex")]
include!(concat!(env!("OUT_DIR"), "/lib.rs"));
