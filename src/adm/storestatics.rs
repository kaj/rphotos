use adm::result::Error;
use brotli2::write::BrotliEncoder;
use flate2::{Compression, GzBuilder};
use std::fs::{create_dir_all, File};
use std::io::prelude::*;
use std::path::Path;
use templates::statics::STATICS;

pub fn to_dir(dir: &str) -> Result<(), Error> {
    let dir: &Path = dir.as_ref();
    create_dir_all(&dir)?;
    for s in STATICS {
        File::create(dir.join(s.name)).and_then(|mut f| f.write(s.content))?;

        let gz = gzipped(s.content)?;
        if gz.len() < s.content.len() {
            File::create(dir.join(format!("{}.gz", s.name)))
                .and_then(|mut f| f.write(&gz))?;
        }
        let br = brcompressed(s.content)?;
        if br.len() < s.content.len() {
            File::create(dir.join(format!("{}.br", s.name)))
                .and_then(|mut f| f.write(&br))?;
        }
    }
    Ok(())
}

fn gzipped(data: &[u8]) -> Result<Vec<u8>, Error> {
    let mut buf = Vec::new();
    {
        let mut gz = GzBuilder::new().write(&mut buf, Compression::best());
        gz.write_all(data)?;
        gz.finish()?;
    }
    Ok(buf)
}

fn brcompressed(data: &[u8]) -> Result<Vec<u8>, Error> {
    let mut buf = Vec::new();
    {
        let mut br = BrotliEncoder::new(&mut buf, 11);
        br.write_all(data)?;
    }
    Ok(buf)
}
