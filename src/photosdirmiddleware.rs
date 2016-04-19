use nickel::{Continue, Middleware, MiddlewareResult, Request, Response};
use plugin::{Extensible};
use typemap::Key;
use photosdir::PhotosDir;
use std::path::PathBuf;

pub struct PhotosDirMiddleware {
    dir: PathBuf,
}

impl PhotosDirMiddleware {
    pub fn new(dir: PathBuf) -> PhotosDirMiddleware {
        PhotosDirMiddleware {
            dir: dir,
        }
    }
}

impl Key for PhotosDirMiddleware { type Value = PhotosDir; }

impl<D> Middleware<D> for PhotosDirMiddleware {
    fn invoke<'mw, 'conn>(&self, req: &mut Request<'mw, 'conn, D>, res: Response<'mw, D>) -> MiddlewareResult<'mw, D> {
        req.extensions_mut().insert::<PhotosDirMiddleware>(
            PhotosDir::new(self.dir.clone()));
        Ok(Continue(res))
    }
}

pub trait PhotosDirRequestExtensions {
    fn photos(&self) -> &PhotosDir;
}

impl<'a, 'b, D> PhotosDirRequestExtensions for Request<'a, 'b, D> {
    fn photos(&self) -> &PhotosDir {
        self.extensions().get::<PhotosDirMiddleware>().unwrap()
    }
}
