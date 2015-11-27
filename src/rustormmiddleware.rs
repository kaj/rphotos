use rustorm::pool::{ManagedPool,Platform};
use rustorm::database::{Database, DbError};
use rustorm::table::IsTable;
use rustorm::dao::{IsDao, ToValue};
use typemap::Key;
use nickel::{Continue, Middleware, MiddlewareResult, Request, Response};
use plugin::{Pluggable, Extensible};

use models::query_for;

pub struct RustormMiddleware {
    pool: ManagedPool
}

impl RustormMiddleware {
    pub fn new(db_url: &str) -> RustormMiddleware {
        RustormMiddleware {
            pool: ManagedPool::init(db_url, 5).unwrap(),
        }
    }
}

impl Key for RustormMiddleware { type Value = Platform; }

impl<D> Middleware<D> for RustormMiddleware {
    fn invoke<'mw, 'conn>(&self, req: &mut Request<'mw, 'conn, D>, res: Response<'mw, D>) -> MiddlewareResult<'mw, D> {
        req.extensions_mut().insert::<RustormMiddleware>(
            self.pool.connect().unwrap());
        Ok(Continue(res))
    }
}

pub trait RustormRequestExtensions {
    fn db_conn(&self) -> &Database;
    fn orm_get<T: IsTable + IsDao>(&self, key: &str, val: &ToValue)
                                   -> Result<T, DbError>;
}

impl<'a, 'b, D> RustormRequestExtensions for Request<'a, 'b, D> {
    fn db_conn(&self) -> &Database {
        self.extensions().get::<RustormMiddleware>().unwrap().as_ref()
    }
    fn orm_get<T: IsTable + IsDao>(&self, key: &str, val: &ToValue)
                                   -> Result<T, DbError> {
        query_for::<T>().filter_eq(key, val).collect_one(self.db_conn())
    }
}
