use super::result::Error;
use crate::templates::statics::STATICS;
use brotli::BrotliCompress;
use brotli::enc::backward_references::BrotliEncoderParams;
use flate2::{Compression, GzBuilder};
use std::fs::{File, create_dir_all};
use std::io::prelude::*;
use std::path::Path;

pub fn to_dir(dir: &str) -> Result<(), Error> {
    let dir: &Path = dir.as_ref();
    for s in STATICS {
        // s.name may contain directory components.
        if let Some(parent) = dir.join(s.name).parent() {
            create_dir_all(parent)?;
        }
        File::create(dir.join(s.name)).and_then(|mut f| f.write(s.content))?;

        let limit = s.content.len() - 10; // Compensate a few bytes overhead
        let gz = gzipped(s.content)?;
        if gz.len() < limit {
            File::create(dir.join(format!("{}.gz", s.name)))
                .and_then(|mut f| f.write(&gz))?;
        }
        let br = brcompressed(s.content)?;
        if br.len() < limit {
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

fn brcompressed(mut data: &[u8]) -> Result<Vec<u8>, Error> {
    let mut buf = Vec::new();
    let params = BrotliEncoderParams {
        quality: 11,
        ..Default::default()
    };
    BrotliCompress(&mut data, &mut buf, &params)?;
    Ok(buf)
}
