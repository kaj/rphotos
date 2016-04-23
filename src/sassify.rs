extern crate sass_rs;
extern crate sass_sys;
extern crate md5;
extern crate rustc_serialize as serialize;

use sass_rs::dispatcher::Dispatcher;
use sass_rs::sass_context::SassFileContext;
use sass_rs::sass_function::*;
use serialize::base64::{self, ToBase64};
use std::env;
use std::fs::{File, create_dir_all};
use std::io::Write;
use std::path::Path;
use std::thread;

fn main() {
    let rod = env::var("OUT_DIR").unwrap();
    let od = Path::new(&rod);
    let css = compile("photos.scss").unwrap();
    let filename = format!("style-{}.css",
                           md5::compute(css.as_bytes())[9..]
                           .to_base64(base64::URL_SAFE));
    let staticdir = od.join("static").join("static");
    create_dir_all(&staticdir).unwrap();
    let mut f = &File::create(&staticdir.join(&*filename)).unwrap();
    write!(f, "{}", css).unwrap();

    let mut link = &File::create(&od.join("stylelink")).unwrap();
    writeln!(link,
             "\"<link rel='stylesheet' href='/static/{}' type='text/css'/>\"",
             filename).unwrap();
}

/// Setup the environment and compile a file.
fn compile(filename:&str) -> Result<String, String> {
    let mut file_context = SassFileContext::new(filename);
    let fns:Vec<(&'static str,Box<SassFunction>)> = vec![];
    let options = file_context.sass_context.sass_options.clone();
    // options.set_output_style(COMPRESSED) or similar, when supported.
    if let Ok(mut opt) = (*options).write() {
        unsafe {
            sass_sys::sass_option_set_output_style(
                opt.raw.get_mut(),
                sass_sys::SASS_STYLE_COMPRESSED);
        }
    }
    thread::spawn(move|| {
        let dispatcher = Dispatcher::build(fns, options);
        while dispatcher.dispatch().is_ok() {}
    });
    file_context.compile()
}
