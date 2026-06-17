use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

/// Holds an exclusive advisory lock on a lockfile for the daemon's lifetime.
/// Dropping it releases the lock.
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    pub fn acquire(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)?;
        file.try_lock_exclusive().map_err(|_| {
            io::Error::new(
                io::ErrorKind::AddrInUse,
                "another servicio daemon is already running",
            )
        })?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_lock_on_same_path_fails_while_first_held() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.lock");
        let first = InstanceLock::acquire(&path).expect("first lock");
        let second = InstanceLock::acquire(&path);
        assert!(second.is_err(), "second acquire must fail while first is held");
        drop(first);
        let third = InstanceLock::acquire(&path);
        assert!(third.is_ok());
    }
}
