extern crate diesel_codegen_syntex as diesel_codegen;
extern crate rsass;
extern crate ructe;

use rsass::{OutputStyle, compile_scss_file};
use ructe::{compile_static_files, compile_templates};
use std::env;
use std::fs::{File, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    prepare_diesel(&out_dir);
    let css_dir = out_dir.join("tmpcss");
    do_sassify(&css_dir);
    let template_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("templates");
    compile_static_files(&css_dir, &out_dir).expect("statics");
    compile_templates(&template_dir, &out_dir).expect("templates");
}

pub fn prepare_diesel(out_dir: &Path) {
    let src = Path::new("src/lib.in.rs");
    let dst = out_dir.join("lib.rs");
    diesel_codegen::expand(&src, &dst).unwrap();

    println!("cargo:rerun-if-changed=src/build.rs");
    println!("cargo:rerun-if-changed=src/models.rs");
    println!("cargo:rerun-if-changed=src/lib.in.rs");
}

pub fn do_sassify(static_dir: &Path) {
    create_dir_all(&static_dir).unwrap();

    let css = compile_scss_file("photos.scss".as_ref(),
                                OutputStyle::Compressed).unwrap();

    File::create(static_dir.join("style.css"))
        .and_then(|mut f| f.write(&css))
        .expect("Writing css");

    // TODO Find any referenced files!
    println!("cargo:rerun-if-changed=photos.scss");
}
