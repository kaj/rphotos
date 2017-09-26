use nickel::status::StatusCode;
use nickel::{Continue, Middleware, MiddlewareResult, Request, Response};
use plugin::Extensible;
use std::sync::{Arc, Mutex};
use time::{Duration, Timespec, get_time};
use typemap::Key;

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
        if ns > 10_000_000 {
            format!("{} ms", ns / 1_000_000)
        } else if ns > 10_000 {
            format!("{} µs", ns / 1000)
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
            RequestLogger::new(mu, Arc::clone(&status)));
        res.on_send(move |r| {
            if let Ok(mut sw) = status.lock() {
                *sw = r.status();
            }
        });
        Ok(Continue(res))
    }
}
