use std::env::var;
use std::process::exit;
use std::path::PathBuf;

pub fn dburl() -> String {
    require_var("DATABASE_URL", "Database url")
}

#[allow(dead_code)]
pub fn jwt_key() -> String {
    require_var("JWT_KEY", "Signing key for jwt")
}

pub fn require_var(name: &str, desc: &str) -> String {
    match var(name) {
        Ok(result) => result,
        Err(error) => {
            println!("{} needed in {}: {}", desc, name, error);
            exit(1);
        }
    }
}

#[allow(dead_code)]
pub fn photos_dir() -> PathBuf {
    PathBuf::from(&*env_or("RPHOTOS_DIR", "/home/kaj/Bilder/foto"))
}

#[allow(dead_code)]
pub fn env_or(name: &str, default: &str) -> String {
    var(name).unwrap_or(default.to_string())
}
