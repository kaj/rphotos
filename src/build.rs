extern crate brotli2;
extern crate flate2;
extern crate md5;
extern crate rustc_serialize as serialize;
extern crate sass_rs;
extern crate sass_sys;

mod sassify;
use sassify::main as do_sassify;

#[cfg(feature = "with-syntex")]
fn main() {
    extern crate syntex;
    extern crate diesel_codegen;
    extern crate dotenv_codegen;

    use std::env;
    use std::path::Path;

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let mut registry = syntex::Registry::new();
    diesel_codegen::register(&mut registry);
    dotenv_codegen::register(&mut registry);

    let src = Path::new("src/lib.in.rs");
    let dst = Path::new(&out_dir).join("lib.rs");

    registry.expand("", &src, &dst).unwrap();

    println!("cargo:rerun-if-changed=src/build.rs");
    println!("cargo:rerun-if-changed=src/models.rs");
    println!("cargo:rerun-if-changed=src/lib.in.rs");
    do_sassify();
}

#[cfg(feature = "nightly")]
fn main() {
    do_sassify();
}
