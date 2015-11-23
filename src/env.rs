use std::env::var;
use std::process::exit;

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
