use crate::models::Photo;
use crate::myexif::ExifData;
use image::imageops::FilterType;
use image::{self, GenericImageView, ImageError, ImageFormat};
use log::{debug, info, warn};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::{fs, io};

pub struct PhotosDir {
    basedir: PathBuf,
}

impl PhotosDir {
    pub fn new(basedir: &Path) -> Self {
        PhotosDir {
            basedir: basedir.into(),
        }
    }

    #[allow(dead_code)]
    pub fn scale_image(
        &self,
        photo: &Photo,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, ImageError> {
        let path = self.basedir.join(&photo.path);
        info!("Should open {:?}", path);
        let img = image::open(path)?;
        let img = if 3 * width <= img.width() || 3 * height <= img.height() {
            img.thumbnail(width, height)
        } else if width < img.width() || height < img.height() {
            img.resize(width, height, FilterType::CatmullRom)
        } else {
            img
        };
        let img = match photo.rotation {
            _x @ 0..=44 | _x @ 315..=360 => img,
            _x @ 45..=134 => img.rotate90(),
            _x @ 135..=224 => img.rotate180(),
            _x @ 225..=314 => img.rotate270(),
            x => {
                warn!("Should rotate photo {} deg, which is unsupported", x);
                img
            }
        };
        let mut buf = Vec::new();
        img.write_to(&mut buf, ImageFormat::Jpeg)?;
        Ok(buf)
    }

    #[allow(dead_code)]
    pub fn get_raw_path(&self, photo: &Photo) -> PathBuf {
        self.basedir.join(&photo.path)
    }

    #[allow(dead_code)]
    pub fn has_file<S: AsRef<OsStr> + ?Sized>(&self, path: &S) -> bool {
        self.basedir.join(Path::new(path)).is_file()
    }

    #[allow(dead_code)]
    pub fn find_files(
        &self,
        dir: &Path,
        cb: &dyn Fn(&str, &ExifData),
    ) -> io::Result<()> {
        let absdir = self.basedir.join(dir);
        if fs::metadata(&absdir)?.is_dir() {
            debug!("Should look in {:?}", absdir);
            for entry in fs::read_dir(absdir)? {
                let path = entry?.path();
                if fs::metadata(&path)?.is_dir() {
                    self.find_files(&path, cb)?;
                } else if let Some(exif) = load_meta(&path) {
                    cb(self.subpath(&path)?, &exif);
                } else {
                    debug!("{:?} is no pic.", path)
                }
            }
        }
        Ok(())
    }

    fn subpath<'a>(&self, fullpath: &'a Path) -> Result<&'a str, io::Error> {
        let path = fullpath
            .strip_prefix(&self.basedir)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        path.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Non-utf8 path {:?}", path),
            )
        })
    }
}

fn load_meta(path: &Path) -> Option<ExifData> {
    if let Ok(mut exif) = ExifData::read_from(&path) {
        if exif.width.is_none() || exif.height.is_none() {
            if let Ok((width, height)) = actual_image_size(&path) {
                exif.width = Some(width);
                exif.height = Some(height);
            }
        }
        Some(exif)
    } else if let Ok((width, height)) = actual_image_size(&path) {
        let mut meta = ExifData::default();
        meta.width = Some(width);
        meta.height = Some(height);
        Some(meta)
    } else {
        None
    }
}

fn actual_image_size(path: &Path) -> Result<(u32, u32), ImageError> {
    let image = image::open(&path)?;
    return Ok((image.width(), image.height()));
}
