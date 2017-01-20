extern crate brotli2;
extern crate diesel_codegen_syntex as diesel_codegen;
extern crate flate2;
extern crate md5;
extern crate rustc_serialize as serialize;
extern crate sass_rs;
extern crate sass_sys;
extern crate ructe;

use brotli2::write::BrotliEncoder;
use flate2::{Compression, FlateWriteExt};
use ructe::compile_templates;
use sass_rs::dispatcher::Dispatcher;
use sass_rs::sass_context::SassFileContext;
use serialize::base64::{self, ToBase64};
use std::env;
use std::fs::{File, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    prepare_diesel(&out_dir);
    do_sassify(&out_dir);
    let template_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("templates");
    compile_templates(&template_dir, &out_dir).unwrap();
}

pub fn prepare_diesel(out_dir: &Path) {
    let src = Path::new("src/lib.in.rs");
    let dst = out_dir.join("lib.rs");
    diesel_codegen::expand(&src, &dst).unwrap();

    println!("cargo:rerun-if-changed=src/build.rs");
    println!("cargo:rerun-if-changed=src/models.rs");
    println!("cargo:rerun-if-changed=src/lib.in.rs");
}

pub fn do_sassify(out_dir: &Path) {
    let static_dir = out_dir.join("static").join("static");
    create_dir_all(&static_dir).unwrap();

    let css = compile("photos.scss").unwrap();
    let css = css.as_bytes();
    let filename = format!("style-{}.css", checksum_slug(&css));

    File::create(static_dir.join(&filename))
        .and_then(|mut f| f.write(css))
        .expect("Writing css");
    File::create(static_dir.join(format!("{}.gz", filename)))
        .map(|f| f.gz_encode(Compression::Best))
        .and_then(|mut f| f.write(css))
        .expect("Writing gzipped css");
    File::create(static_dir.join(format!("{}.br", filename)))
        .map(|f| BrotliEncoder::new(f, 11))
        .and_then(|mut f| f.write(css))
        .expect("Writing brotli compressed css");

    File::create(&out_dir.join("stylelink"))
        .and_then(|mut f| {
            writeln!(f,
                     "\"<link rel='stylesheet' href='/static/{}' \
                      type='text/css'/>\"",
                     filename)
        })
        .expect("Writing stylelink");

    // TODO Find any referenced files!
    println!("cargo:rerun-if-changed=photos.scss");
}

/// A short and url-safe checksum string from string data.
fn checksum_slug(data: &[u8]) -> String {
    md5::compute(data)[9..].to_base64(base64::URL_SAFE)
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
