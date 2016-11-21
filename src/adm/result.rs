use diesel::prelude::ConnectionError;
use diesel::result::Error as DieselError;
use std::convert::From;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    Connection(ConnectionError),
    Db(DieselError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::Connection(ref e) => write!(f, "Connection error: {}", e),
            &Error::Db(ref e) => write!(f, "Database error: {}", e),
        }
    }
}

impl From<ConnectionError> for Error {
    fn from(e: ConnectionError) -> Self {
        Error::Connection(e)
    }
}

impl From<DieselError> for Error {
    fn from(e: DieselError) -> Self {
        Error::Db(e)
    }
}
