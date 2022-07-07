use crate::Error;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::{Connection, ConnectionError};
use log::debug;
use std::time::{Duration, Instant};

pub type PgPool = Pool<ConnectionManager<PgConnection>>;
pub type PooledPg = PooledConnection<ConnectionManager<PgConnection>>;

#[derive(clap::Parser)]
pub struct DbOpt {
    /// How to connect to the postgres database.
    #[clap(long, env = "DATABASE_URL", hide_env_values = true)]
    db_url: String,
}

impl DbOpt {
    pub fn connect(&self) -> Result<PgConnection, ConnectionError> {
        let time = Instant::now();
        let db = PgConnection::establish(&self.db_url)?;
        debug!("Got db connection in {:?}", time.elapsed());
        Ok(db)
    }
    pub fn create_pool(&self) -> Result<PgPool, Error> {
        let time = Instant::now();
        let pool = Pool::builder()
            .min_idle(Some(2))
            .test_on_check_out(false)
            .connection_timeout(Duration::from_millis(500))
            .build(ConnectionManager::new(&self.db_url))?;
        debug!("Created pool in {:?}", time.elapsed());
        Ok(pool)
    }
}
