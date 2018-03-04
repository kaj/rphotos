use image::{self, FilterType, GenericImage, ImageError, ImageFormat};
use models::Photo;
use myexif::ExifData;
use std::{fs, io};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub struct PhotosDir {
    basedir: PathBuf,
}

impl PhotosDir {
    pub fn new(basedir: PathBuf) -> Self {
        PhotosDir { basedir: basedir }
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
        let img = if width < img.width() || height < img.height() {
            img.resize(width, height, FilterType::CatmullRom)
        } else {
            img
        };
        let img = match photo.rotation {
            _x @ 0...44 | _x @ 315...360 => img,
            _x @ 45...134 => img.rotate90(),
            _x @ 135...224 => img.rotate180(),
            _x @ 225...314 => img.rotate270(),
            x => {
                warn!("Should rotate photo {} deg, which is unsupported", x);
                img
            }
        };
        let mut buf = Vec::new();
        img.save(&mut buf, ImageFormat::JPEG)?;
        Ok(buf)
    }

    #[allow(dead_code)]
    pub fn get_raw_path(&self, photo: Photo) -> PathBuf {
        self.basedir.join(photo.path)
    }

    #[allow(dead_code)]
    pub fn has_file<S: AsRef<OsStr> + ?Sized>(&self, path: &S) -> bool {
        self.basedir.join(Path::new(path)).is_file()
    }

    #[allow(dead_code)]
    pub fn find_files(
        &self,
        dir: &Path,
        cb: &Fn(&str, &ExifData),
    ) -> io::Result<()> {
        let absdir = self.basedir.join(dir);
        if fs::metadata(&absdir)?.is_dir() {
            let bl = self.basedir.to_str().unwrap().len() + 1;
            debug!("Should look in {:?}", absdir);
            for entry in fs::read_dir(absdir)? {
                let path = entry?.path();
                if fs::metadata(&path)?.is_dir() {
                    self.find_files(&path, cb)?;
                } else if let Ok(exif) = ExifData::read_from(&path) {
                    let path = path.to_str().unwrap();
                    cb(&path[bl..], &exif);
                } else if image::open(&path).is_ok() {
                    let none = ExifData::default();
                    info!("{:?} seems like a pic with no exif.", path);
                    let path = path.to_str().unwrap();
                    cb(&path[bl..], &none);
                } else {
                    debug!("{:?} is no pic.", path)
                }
            }
        }
        Ok(())
    }
}
