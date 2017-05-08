extern crate diesel_codegen_syntex as diesel_codegen;
extern crate ructe;

use ructe::{StaticFiles, compile_templates};
use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    prepare_diesel(&out_dir);

    let base_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let template_dir = base_dir.join("templates");
    let mut statics = StaticFiles::new(&out_dir).unwrap();
    statics.add_sass_file(&base_dir.join("photos.scss")).unwrap();
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
