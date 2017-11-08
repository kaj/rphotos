use diesel::connection::Connection;
use nickel::{Continue, Middleware, MiddlewareResult, Request, Response};
use plugin::Extensible;
use r2d2::{Config, HandleError, Pool, PooledConnection};
use r2d2_diesel::ConnectionManager;
use std::any::Any;
use std::error::Error as StdError;
use std::sync::Arc;
use typemap::Key;


pub struct DieselMiddleware<T>
where
    T: Connection + Send + Any,
{
    pub pool: Arc<Pool<ConnectionManager<T>>>,
}

impl<T> DieselMiddleware<T>
where
    T: Connection + Send + Any,
{
    pub fn new(
        connect_str: &str,
        num_connections: u32,
        error_handler: Box<HandleError<::r2d2_diesel::Error>>,
    ) -> Result<DieselMiddleware<T>, Box<StdError>> {
        let manager = ConnectionManager::<T>::new(connect_str);

        let config = Config::builder()
            .pool_size(num_connections)
            .error_handler(error_handler)
            .build();

        let pool = Pool::new(config, manager)?;

        Ok(DieselMiddleware { pool: Arc::new(pool) })
    }

    #[allow(dead_code)]
    pub fn from_pool(pool: Pool<ConnectionManager<T>>) -> DieselMiddleware<T> {
        DieselMiddleware { pool: Arc::new(pool) }
    }
}

impl<T> Key for DieselMiddleware<T>
where
    T: Connection + Send + Any,
{
    type Value = Arc<Pool<ConnectionManager<T>>>;
}

impl<T, D> Middleware<D> for DieselMiddleware<T>
where
    T: Connection + Send + Any,
{
    fn invoke<'mw, 'conn>(
        &self,
        req: &mut Request<'mw, 'conn, D>,
        res: Response<'mw, D>,
    ) -> MiddlewareResult<'mw, D> {
        req.extensions_mut()
            .insert::<DieselMiddleware<T>>(Arc::clone(&self.pool));
        Ok(Continue(res))
    }
}

pub trait DieselRequestExtensions<T>
where
    T: Connection + Send + Any,
{
    fn db_conn(&self) -> PooledConnection<ConnectionManager<T>>;
}

impl<'a, 'b, T, D> DieselRequestExtensions<T> for Request<'a, 'b, D>
where
    T: Connection + Send + Any,
{
    fn db_conn(&self) -> PooledConnection<ConnectionManager<T>> {
        self.extensions()
            .get::<DieselMiddleware<T>>()
            .unwrap()
            .get()
            .unwrap()
    }
}
