use crate::Error;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::{Connection, ConnectionError};
use std::time::Duration;
use structopt::StructOpt;

pub type PgPool = Pool<ConnectionManager<PgConnection>>;
pub type PooledPg = PooledConnection<ConnectionManager<PgConnection>>;

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct DbOpt {
    /// How to connect to the postgres database.
    #[structopt(long, env = "DATABASE_URL", hide_env_values = true)]
    db_url: String,
}

impl DbOpt {
    pub fn connect(&self) -> Result<PgConnection, ConnectionError> {
        PgConnection::establish(&self.db_url)
    }
    pub fn create_pool(&self) -> Result<PgPool, Error> {
        let db_manager = ConnectionManager::<PgConnection>::new(&self.db_url);
        Ok(Pool::builder()
            .connection_timeout(Duration::from_secs(1))
            .build(db_manager)?)
    }
}
