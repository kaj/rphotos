extern crate ructe;

use ructe::{compile_templates, StaticFiles};
use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let base_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let mut statics = StaticFiles::new(&out_dir).unwrap();
    let s_dir = base_dir.join("res");
    statics.add_sass_file(&s_dir.join("photos.scss")).unwrap();
    statics.add_file(&s_dir.join("admin.js")).unwrap();
    statics.add_file(&s_dir.join("ux.js")).unwrap();
    statics.add_files_as(&s_dir.join("leaflet-1.3.1"), "l131").unwrap();
    compile_templates(&base_dir.join("templates"), &out_dir).unwrap();
}
