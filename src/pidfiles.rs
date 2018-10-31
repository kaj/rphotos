use libc::{getpid, kill, pid_t, SIGHUP};
use log::{debug, info};
use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::path::Path;

pub fn handle_pid_file(pidfile: &str, replace: bool) -> Result<(), String> {
    if replace {
        if let Some(oldpid) = read_pid_file(pidfile)? {
            info!("Killing old pid {}.", oldpid);
            unsafe {
                kill(oldpid, SIGHUP);
            }
        }
    } else if Path::new(pidfile).exists() {
        return Err(format!("Pid file {:?} exists.", pidfile));
    }
    let pid = unsafe { getpid() };
    debug!("Should write pid {} to {}", pid, pidfile);
    File::create(pidfile)
        .and_then(|mut f| writeln!(f, "{}", pid))
        .map_err(|e| format!("Failed to write {}: {}", pidfile, e))
}

fn read_pid_file(pidfile: &str) -> Result<Option<pid_t>, String> {
    match File::open(pidfile) {
        Ok(mut f) => {
            let mut buf = String::new();
            f.read_to_string(&mut buf)
                .map_err(|e| format!("Could not read {}: {}", pidfile, e))?;
            Ok(Some(buf.trim().parse().map_err(|e| {
                format!("Bad content in {}: {}", pidfile, e)
            })?))
        }
        Err(ref e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(ref e) => Err(format!("Could not open {}: {}", pidfile, e)),
    }
}
