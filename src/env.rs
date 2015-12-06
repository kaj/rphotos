use std::env::var;
use std::process::exit;
use std::path::PathBuf;

pub fn dburl() -> String {
    let db_var = "RPHOTOS_DB";
    match var(db_var) {
        Ok(result) => result,
        Err(error) => {
            println!("A database url needs to be given in env {}: {}",
                     db_var, error);
            exit(1);
        }
    }
}

pub fn photos_dir() -> PathBuf {
    PathBuf::from(&*env_or("RPHOTOS_DIR", "/home/kaj/Bilder/foto"))
}

pub fn env_or(name: &str, default: &str) -> String {
    var(name).unwrap_or(default.to_string())
}
