use nickel::{Continue, Middleware, MiddlewareResult, Request, Response};
use nickel::status::StatusCode;
use plugin::Extensible;
use typemap::Key;
use time::{Duration, Timespec, get_time};
use std::sync::{Arc, Mutex};

pub struct RequestLoggerMiddleware;

pub struct RequestLogger {
    start: Timespec,
    mu: String,
    status: Arc<Mutex<StatusCode>>,
}

impl RequestLogger {
    pub fn new(mu: String, status: Arc<Mutex<StatusCode>>) -> RequestLogger {
        debug!("Start handling {}", mu);
        RequestLogger {
            start: get_time(),
            mu: mu,
            status: status,
        }
    }
}

impl Drop for RequestLogger {
    fn drop(&mut self) {
        if let Ok(status) = self.status.lock() {
            info!("{:?} {} after {}",
                  self.mu,
                  *status,
                  fmt_elapsed(get_time() - self.start));
        }
    }
}

fn fmt_elapsed(t: Duration) -> String {
    let ms = t.num_milliseconds();
    if ms > 1000 {
        format!("{:.2} s", ms as f32 * 1e-3)
    } else {
        let ns = t.num_nanoseconds().unwrap();
        if ns > 1e6 as i64 {
            format!("{} ms", ns / 1e6 as i64)
        } else if ns > 1000 {
            format!("{} us", ns / 1000)
        } else {
            format!("{} ns", ns)
        }
    }
}

impl Key for RequestLoggerMiddleware {
    type Value = RequestLogger;
}

impl<D> Middleware<D> for RequestLoggerMiddleware {
    fn invoke<'mw, 'conn>(&self,
                          req: &mut Request<'mw, 'conn, D>,
                          mut res: Response<'mw, D>)
                          -> MiddlewareResult<'mw, D> {
        let mu = format!("{} {}", req.origin.method, req.origin.uri);
        let status = Arc::new(Mutex::new(StatusCode::Continue));
        req.extensions_mut().insert::<RequestLoggerMiddleware>(
            RequestLogger::new(mu, status.clone()));
        res.on_send(move |r| {
            if let Ok(mut sw) = status.lock() {
                *sw = r.status();
            }
        });
        Ok(Continue(res))
    }
}
