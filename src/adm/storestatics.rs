use adm::result::Error;
use brotli2::write::BrotliEncoder;
use flate2::{Compression, FlateWriteExt};
use std::fs::{File, create_dir_all};
use std::io::prelude::*;
use std::path::Path;

pub fn to_dir(dir: &str) -> Result<(), Error> {
    let dir: &Path = dir.as_ref();
    try!(create_dir_all(&dir));
    for s in STATICS {
        try!(File::create(dir.join(s.name))
            .and_then(|mut f| f.write(s.content)));

        try!(File::create(dir.join(format!("{}.gz", s.name)))
            .map(|f| f.gz_encode(Compression::Best))
            .and_then(|mut f| f.write(s.content)));

        try!(File::create(dir.join(format!("{}.br", s.name)))
            .map(|f| BrotliEncoder::new(f, 11))
            .and_then(|mut f| f.write(s.content)));
    }
    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/templates/statics.rs"));
