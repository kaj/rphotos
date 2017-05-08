use image::{self, FilterType, GenericImage, ImageError, ImageFormat};
use rexif::{self, ExifData};
use rphotos::models::Photo;
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
    pub fn get_scaled_image(&self,
                            photo: Photo,
                            width: u32,
                            height: u32)
                            -> Result<Vec<u8>, ImageError> {
        let path = self.basedir.join(photo.path);
        info!("Should open {:?}", path);
        let img = image::open(path)?;
        let img = if width < img.width() || height < img.height() {
            img.resize(width, height, FilterType::CatmullRom)
        } else {
            img
        };
        let img = match photo.rotation {
            _x @ 0...44 => img,
            _x @ 45...134 => img.rotate90(),
            _x @ 135...224 => img.rotate180(),
            _x @ 225...314 => img.rotate270(),
            _x @ 315...360 => img,
            x => {
                warn!("Should rotate photo {} deg, which is unsupported", x);
                img
            }
        };
        // TODO Put the icon in some kind of cache!
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
    pub fn find_files(&self,
                      dir: &Path,
                      cb: &Fn(&str, &ExifData))
                      -> io::Result<()> {
        let absdir = self.basedir.join(dir);
        if fs::metadata(&absdir)?.is_dir() {
            let bl = self.basedir.to_str().unwrap().len() + 1;
            debug!("Should look in {:?}", absdir);
            for entry in fs::read_dir(absdir)? {
                let entry = entry?;
                if fs::metadata(entry.path())?.is_dir() {
                    self.find_files(&entry.path(), cb)?;
                } else {
                    let p1 = entry.path();
                    if let Ok(exif) = rexif::parse_file(&p1.to_str().unwrap()) {
                        let path = p1.to_str().unwrap();
                        cb(&path[bl..], &exif);
                    } else {
                        if image::open(p1.clone()).is_ok() {
                            let none = ExifData {
                                mime: "".into(),
                                entries: vec![],
                            };
                            info!("{:?} seems like a pic with no exif.", p1);
                            let path = p1.to_str().unwrap();
                            cb(&path[bl..], &none);
                        } else {
                            debug!("{:?} is no pic.", p1)
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
