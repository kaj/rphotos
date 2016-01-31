use std::path::{Path, PathBuf};
use std::fs;
use std::io;
use image::open as image_open;
use image::{FilterType, ImageFormat, GenericImage};
use rexif::{self, ExifData};

use models::Photo;

pub struct PhotosDir {
    basedir: PathBuf
}

impl PhotosDir {
    pub fn new(basedir: PathBuf) -> PhotosDir {
        PhotosDir {
            basedir: basedir
        }
    }

    pub fn get_scaled_image(&self, photo: Photo, width: u32, height: u32)
                        -> Vec<u8> {
        let path = self.basedir.join(photo.path);
        info!("Should open {:?}", path);
        let img = image_open(path).unwrap();
        let img =
            if width < img.width() || height < img.height() {
                img.resize(width, height, FilterType::Nearest)
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
        let mut buf : Vec<u8> = Vec::new();
        img.save(&mut buf, ImageFormat::JPEG).unwrap();
        buf
    }

    pub fn find_files(&self, dir: &Path, cb: &Fn(&str, &ExifData)) -> io::Result<()> {
        let absdir = self.basedir.join(dir);
        if try!(fs::metadata(&absdir)).is_dir() {
            let bl = self.basedir.to_str().unwrap().len() + 1;
            debug!("Should look in {:?}", absdir);
            for entry in try!(fs::read_dir(absdir)) {
                let entry = try!(entry);
                if try!(fs::metadata(entry.path())).is_dir() {
                    try!(self.find_files(&entry.path(), cb));
                } else {
                    let p1 = entry.path();
                    if let Ok(exif) = rexif::parse_file(&p1.to_str().unwrap()) {
                        let path = p1.to_str().unwrap();
                        cb(&path[bl..], &exif);
                    }
                }
            }
        }
        Ok(())
    }
}
