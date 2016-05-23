extern crate brotli2;
extern crate flate2;
extern crate md5;
extern crate rustc_serialize as serialize;
extern crate sass_rs;
extern crate sass_sys;

use brotli2::write::BrotliEncoder;
use flate2::{Compression, FlateWriteExt};
use sass_rs::dispatcher::Dispatcher;
use sass_rs::sass_context::SassFileContext;
use serialize::base64::{self, ToBase64};
use std::env;
use std::fs::{File, create_dir_all};
use std::io::Write;
use std::path::PathBuf;
use std::thread;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let static_dir = out_dir.join("static").join("static");
    create_dir_all(&static_dir).unwrap();

    let css = compile("photos.scss").unwrap();
    let filename = format!("style-{}.css", checksum_slug(&css));

    File::create(&static_dir.join(&filename))
        .map(|mut f| {
            write!(f, "{}", css).unwrap();
        })
        .unwrap();
    File::create(&static_dir.join(format!("{}.gz", &filename)))
        .map(|f| {
            write!(f.gz_encode(Compression::Best), "{}", css).unwrap();
        })
        .unwrap();
    File::create(&static_dir.join(format!("{}.br", &filename)))
        .map(|f| {
            write!(BrotliEncoder::new(f, 11), "{}", css).unwrap();
        })
        .unwrap();

    File::create(&out_dir.join("stylelink"))
        .map(|mut f| {
            writeln!(f,
                     "\"<link rel='stylesheet' href='/static/{}' \
                      type='text/css'/>\"",
                     filename)
                .unwrap();
        })
        .unwrap();

    println!("cargo:rerun-if-changed=src/sassify.rs");
    // TODO Find any referenced files!
    println!("cargo:rerun-if-changed=photos.scss");
}

/// A short and url-safe checksum string from string data.
fn checksum_slug(data: &str) -> String {
    md5::compute(data.as_bytes())[9..].to_base64(base64::URL_SAFE)
}

/// Setup the sass environment and compile a file.
fn compile(filename: &str) -> Result<String, String> {
    let mut file_context = SassFileContext::new(filename);
    // options.set_output_style(COMPRESSED) or similar, when supported.
    if let Ok(mut opt) = file_context.sass_context.sass_options.write() {
        unsafe {
            sass_sys::sass_option_set_output_style(
                opt.raw.get_mut(),
                sass_sys::SASS_STYLE_COMPRESSED);
        }
    }
    let options = file_context.sass_context.sass_options.clone();
    thread::spawn(move || {
        let dispatcher = Dispatcher::build(vec![], options);
        while dispatcher.dispatch().is_ok() {}
    });
    file_context.compile()
}
