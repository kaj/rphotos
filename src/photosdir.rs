use crate::models::Photo;
use crate::myexif::ExifData;
use async_walkdir::WalkDir;
use image::imageops::FilterType;
use image::{self, DynamicImage, ImageError, ImageFormat};
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::task::{JoinError, spawn_blocking};
use tracing::{debug, info, instrument, warn};

pub struct PhotosDir {
    basedir: PathBuf,
}

impl PhotosDir {
    pub fn new(basedir: &Path) -> Self {
        PhotosDir {
            basedir: basedir.into(),
        }
    }

    pub fn get_raw_path(&self, photo: &Photo) -> PathBuf {
        self.basedir.join(&photo.path)
    }

    pub fn has_file<S: AsRef<OsStr> + ?Sized>(&self, path: &S) -> bool {
        self.basedir.join(Path::new(path)).is_file()
    }

    pub fn walk_dir(&self, dir: &Path) -> WalkDir {
        WalkDir::new(self.basedir.join(dir))
    }

    pub fn subpath<'a>(
        &self,
        fullpath: &'a Path,
    ) -> Result<&'a str, io::Error> {
        let path = fullpath
            .strip_prefix(&self.basedir)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        path.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Non-utf8 path {path:?}"),
            )
        })
    }
}

#[instrument]
pub fn load_meta(path: &Path) -> Option<ExifData> {
    if let Ok(mut exif) = ExifData::read_from(path) {
        if exif.width.is_none() || exif.height.is_none() {
            if let Ok((width, height)) = actual_image_size(path) {
                exif.width = Some(width);
                exif.height = Some(height);
            }
        }
        Some(exif)
    } else if let Ok((width, height)) = actual_image_size(path) {
        let mut meta = ExifData::default();
        meta.width = Some(width);
        meta.height = Some(height);
        Some(meta)
    } else {
        None
    }
}

fn actual_image_size(path: &Path) -> Result<(u32, u32), ImageError> {
    let image = image::open(path)?;
    Ok((image.width(), image.height()))
}

#[derive(Debug)]
pub enum ImageLoadFailed {
    File(io::Error),
    Image(image::ImageError),
    Join(JoinError),
}

impl std::error::Error for ImageLoadFailed {}

impl std::fmt::Display for ImageLoadFailed {
    fn fmt(&self, out: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self {
            ImageLoadFailed::File(e) => e.fmt(out),
            ImageLoadFailed::Image(e) => e.fmt(out),
            ImageLoadFailed::Join(e) => e.fmt(out),
        }
    }
}

impl From<io::Error> for ImageLoadFailed {
    fn from(e: io::Error) -> ImageLoadFailed {
        ImageLoadFailed::File(e)
    }
}
impl From<image::ImageError> for ImageLoadFailed {
    fn from(e: image::ImageError) -> ImageLoadFailed {
        ImageLoadFailed::Image(e)
    }
}
impl From<JoinError> for ImageLoadFailed {
    fn from(e: JoinError) -> ImageLoadFailed {
        ImageLoadFailed::Join(e)
    }
}

pub async fn get_scaled_jpeg(
    path: PathBuf,
    rotation: i16,
    size: u16,
) -> Result<Vec<u8>, ImageLoadFailed> {
    spawn_blocking(move || do_get_scaled_jpeg(path, rotation, size)).await?
}

#[tracing::instrument]
fn do_get_scaled_jpeg(
    path: PathBuf,
    rotation: i16,
    size: u16,
) -> Result<Vec<u8>, ImageLoadFailed> {
    let start = Instant::now();
    let img = if is_jpeg(&path) {
        let file = BufReader::new(File::open(path)?);
        let mut decoder = image::codecs::jpeg::JpegDecoder::new(file)?;
        decoder.scale(size, size)?;
        DynamicImage::from_decoder(decoder)?
    } else {
        image::open(path)?
    };

    let size = u32::from(size);
    debug!(size = %Size(&img), elapsed = ?start.elapsed(), "Loaded image.");
    let img = if 3 * size <= img.width() || 3 * size <= img.height() {
        img.thumbnail(size, size)
    } else if size < img.width() || size < img.height() {
        img.resize(size, size, FilterType::CatmullRom)
    } else {
        img
    };
    debug!(size = %Size(&img), elapsed = ?start.elapsed(), "Scaled image.");
    let img = match rotation {
        _x @ (0..=44 | 315..=360) => img,
        _x @ 45..=134 => img.rotate90(),
        _x @ 135..=224 => img.rotate180(),
        _x @ 225..=314 => img.rotate270(),
        x => {
            warn!("Should rotate photo {} deg, which is unsupported.", x);
            img
        }
    };
    debug!(elapsed = ?start.elapsed(), "Ready to save.");
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Jpeg)?;
    info!(elapsed = ?start.elapsed(), "Done.");
    Ok(buf)
}

struct Size<'a>(&'a DynamicImage);
impl std::fmt::Display for Size<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.0.width(), self.0.height())
    }
}

fn is_jpeg(path: &Path) -> bool {
    if let Some(suffix) = path.extension().and_then(|s| s.to_str()) {
        suffix.eq_ignore_ascii_case("jpg")
            || suffix.eq_ignore_ascii_case("jpeg")
    } else {
        false
    }
}
