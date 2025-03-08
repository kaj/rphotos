use crate::Error;
use diesel::ConnectionError;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::deadpool;
use diesel_async::{AsyncConnection, AsyncPgConnection};
use std::time::Instant;
use tracing::debug;

/// An asynchronous postgres database connection pool.
pub type PgPool = deadpool::Pool<AsyncPgConnection>;
pub type PooledPg = deadpool::Object<AsyncPgConnection>;

#[derive(clap::Parser)]
pub struct DbOpt {
    /// How to connect to the postgres database.
    #[clap(long, env = "DATABASE_URL", hide_env_values = true)]
    db_url: String,
}

impl DbOpt {
    pub async fn connect(&self) -> Result<AsyncPgConnection, ConnectionError> {
        let time = Instant::now();
        let db = AsyncPgConnection::establish(&self.db_url).await?;
        debug!("Got db connection in {:?}", time.elapsed());
        Ok(db)
    }
    pub fn create_pool(&self) -> Result<PgPool, Error> {
        let time = Instant::now();
        let config = AsyncDieselConnectionManager::new(&self.db_url);
        let pool = PgPool::builder(config)
            .build()
            .map_err(|e| Error::Other(format!("Pool creating error: {e}")))?;
        debug!("Created pool in {:?}", time.elapsed());
        Ok(pool)
    }
}
