use nickel::{Continue, Middleware, MiddlewareResult, Request, Response};
use nickel::status::StatusCode;
use plugin::{Extensible};
use typemap::Key;
use time::{Timespec,get_time};
use std::sync::{Arc, Mutex};

pub struct RequestLoggerMiddleware;

pub struct RequestLogger {
    start: Timespec,
    mu: String,
    status: Arc<Mutex<StatusCode>>
}

impl RequestLogger {
    pub fn new(mu: String, status: Arc<Mutex<StatusCode>>) -> RequestLogger {
        debug!("Start handling {}", mu);
        RequestLogger {
            start: get_time(),
            mu: mu,
            status: status
        }
    }
}

impl Drop for RequestLogger {
    fn drop(&mut self) {
        let status = self.status.lock().unwrap();
        info!("{} {} after {}", self.mu, *status, get_time() - self.start);
    }
}

impl Key for RequestLoggerMiddleware { type Value = RequestLogger; }

impl<D> Middleware<D> for RequestLoggerMiddleware {
    fn invoke<'mw, 'conn>(&self, req: &mut Request<'mw, 'conn, D>, res: Response<'mw, D>) -> MiddlewareResult<'mw, D> {
        let mu = format!("\"{} {}\"", req.origin.method, req.origin.uri);
        let status = Arc::new(Mutex::new(StatusCode::Continue));
        let rl = RequestLogger::new(mu, status.clone());
        req.extensions_mut().insert::<RequestLoggerMiddleware>(rl);
        let mut r2 = res; // How strange is this?!?
        r2.on_send(move |r| {
            let mut sw = status.lock().unwrap();
            *sw = r.status();
        });
        Ok(Continue(r2))
    }
}
