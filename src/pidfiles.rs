use crate::adm::result::Error;
use libc::{kill, pid_t, SIGHUP};
use log::{debug, info};
use std::fs::{read_to_string, write};
use std::io::ErrorKind;
use std::path::Path;
use std::process;

pub fn handle_pid_file(pidfile: &Path, replace: bool) -> Result<(), Error> {
    if replace {
        if let Some(oldpid) = read_pid_file(pidfile)? {
            info!("Killing old pid {}.", oldpid);
            unsafe {
                kill(oldpid, SIGHUP);
            }
        }
    } else if pidfile.exists() {
        return Err(Error::Other(format!("Pid file {:?} exists.", pidfile)));
    }
    let pid = process::id();
    debug!("Should write pid {} to {:?}", pid, pidfile);
    write(pidfile, pid.to_string()).map_err(|e| Error::in_file(&e, pidfile))
}

fn read_pid_file(pidfile: &Path) -> Result<Option<pid_t>, Error> {
    match read_to_string(pidfile) {
        Ok(pid) => pid
            .trim()
            .parse()
            .map(Some)
            .map_err(|e| Error::in_file(&e, pidfile)),
        Err(ref e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(ref e) => Err(Error::in_file(&e, pidfile)),
    }
}
