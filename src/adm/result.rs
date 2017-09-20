use chrono::ParseError as ChronoParseError;
use diesel::prelude::ConnectionError;
use diesel::result::Error as DieselError;
use memcached::proto::Error as MemcachedError;
use std::{io, fmt};
use std::convert::From;
use std::num::ParseIntError;

#[derive(Debug)]
pub enum Error {
    Connection(ConnectionError),
    Db(DieselError),
    Io(io::Error),
    UnknownOrientation(u16),
    BadTimeFormat(ChronoParseError),
    BadIntFormat(ParseIntError),
    Cache(MemcachedError),
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::Connection(ref e) => write!(f, "Connection error: {}", e),
            &Error::Db(ref e) => write!(f, "Database error: {}", e),
            &Error::Io(ref e) => write!(f, "I/O error: {}", e),
            &Error::UnknownOrientation(ref o) => {
                write!(f, "Unknown image orientation: {:?}", o)
            }
            &Error::BadTimeFormat(ref e) => write!(f, "Bad time value: {}", e),
            &Error::BadIntFormat(ref e) => write!(f, "Bad int value: {}", e),
            &Error::Cache(ref e) => write!(f, "Memcached error: {}", e),
            &Error::Other(ref s) => write!(f, "Error: {}", s),
        }
    }
}

impl From<MemcachedError> for Error {
    fn from(e: MemcachedError) -> Self {
        Error::Cache(e)
    }
}

impl From<ConnectionError> for Error {
    fn from(e: ConnectionError) -> Self {
        Error::Connection(e)
    }
}

impl From<ChronoParseError> for Error {
    fn from(e: ChronoParseError) -> Self {
        Error::BadTimeFormat(e)
    }
}

impl From<ParseIntError> for Error {
    fn from(e: ParseIntError) -> Self {
        Error::BadIntFormat(e)
    }
}

impl From<DieselError> for Error {
    fn from(e: DieselError) -> Self {
        Error::Db(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}
