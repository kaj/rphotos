use memcached::Client;
use memcached::proto::{Operation, ProtoType, Error as MprotError};
use nickel::{Continue, Middleware, MiddlewareResult, Request, Response};
use plugin::Extensible;
use std::{fmt, io};
use std::convert::From;
use std::error::Error;
use typemap::Key;

pub struct MemcacheMiddleware {
    servers: Vec<(String, usize)>,
}

impl MemcacheMiddleware {
    pub fn new(servers: Vec<(String, usize)>) -> Self {
        MemcacheMiddleware { servers: servers }
    }
}

impl Key for MemcacheMiddleware {
    type Value = Vec<(String, usize)>;
}

impl<D> Middleware<D> for MemcacheMiddleware {
    fn invoke<'mw, 'conn>(&self,
                          req: &mut Request<'mw, 'conn, D>,
                          res: Response<'mw, D>)
                          -> MiddlewareResult<'mw, D> {
        req.extensions_mut()
            .insert::<MemcacheMiddleware>(self.servers.clone());
        Ok(Continue(res))
    }
}

pub trait MemcacheRequestExtensions {
    fn cache(&self) -> Result<Client, McError>;

    fn cached_or<F, E>(&self, key: &str, calculate: F) -> Result<Vec<u8>, E>
        where F: FnOnce() -> Result<Vec<u8>, E>;
    fn clear_cache(&self, key: &str);
}

#[derive(Debug)]
pub enum McError {
    UninitializedMiddleware,
    IoError(io::Error),
}

impl fmt::Display for McError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &McError::UninitializedMiddleware => {
                write!(f, "middleware not properly initialized")
            }
            &McError::IoError(ref err) => write!(f, "{}", err.description()),
        }
    }
}

impl Error for McError {
    fn description(&self) -> &str {
        match self {
            &McError::UninitializedMiddleware => {
                "middleware not properly initialized"
            }
            &McError::IoError(ref err) => err.description(),
        }
    }
}

impl From<io::Error> for McError {
    fn from(err: io::Error) -> Self {
        McError::IoError(err)
    }
}

impl<'a, 'b, D> MemcacheRequestExtensions for Request<'a, 'b, D> {
    fn cache(&self) -> Result<Client, McError> {
        match self.extensions().get::<MemcacheMiddleware>() {
            Some(ext) => {
                let mut servers = Vec::new();
                for &(ref s, n) in ext {
                    servers.push((&s[..], n));
                }
                Ok(Client::connect(&servers, ProtoType::Binary)?)
            }
            None => Err(McError::UninitializedMiddleware),
        }
    }

    fn cached_or<F, E>(&self, key: &str, init: F) -> Result<Vec<u8>, E>
        where F: FnOnce() -> Result<Vec<u8>, E>
    {
        match self.cache() {
            Ok(mut client) => {
                match client.get(&key.as_bytes()) {
                    Ok((data, _flags)) => {
                        debug!("Cache: {} found", key);
                        return Ok(data);
                    }
                    Err(MprotError::BinaryProtoError(ref err))
                        if err.description() ==
                           "key not found" => {
                        debug!("Cache: {} not found", key);
                    }
                    Err(err) => {
                        warn!("Cache: get {} failed: {:?}", key, err);
                    }
                }
                let data = init()?;
                match client.set(key.as_bytes(), &data, 0, 7 * 24 * 60 * 60) {
                    Ok(()) => debug!("Cache: stored {}", key),
                    Err(err) => warn!("Cache: Error storing {}: {}", key, err),
                }
                Ok(data)
            }
            Err(err) => {
                warn!("Error connecting to memcached: {}", err);
                init()
            }
        }
    }

    fn clear_cache(&self, key: &str) {
        if let Ok(mut client) = self.cache() {
            match client.delete(key.as_bytes()) {
                Ok(()) => debug!("Cache: deleted {}", key),
                Err(e) => warn!("Cache: Failed to delete {}: {}", key, e),
            }
        }
    }
}
