use std::path::PathBuf;

/// Resolved filesystem locations for one daemon instance, all under a base dir.
#[derive(Debug, Clone)]
pub struct Paths {
    pub base: PathBuf,
}

impl Paths {
    pub fn new(base: PathBuf) -> Self {
        Self { base }
    }

    /// Default base: $XDG_RUNTIME_DIR/servicio, else a temp-dir fallback.
    pub fn default_base() -> PathBuf {
        if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
            PathBuf::from(dir).join("servicio")
        } else {
            std::env::temp_dir().join("servicio")
        }
    }

    pub fn socket(&self) -> PathBuf {
        self.base.join("daemon.sock")
    }
    pub fn token(&self) -> PathBuf {
        self.base.join("token")
    }
    pub fn lock(&self) -> PathBuf {
        self.base.join("daemon.lock")
    }
    pub fn db(&self) -> PathBuf {
        self.base.join("servicio.db")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_under_base() {
        let p = Paths::new(PathBuf::from("/tmp/x"));
        assert_eq!(p.socket(), PathBuf::from("/tmp/x/daemon.sock"));
        assert_eq!(p.token(), PathBuf::from("/tmp/x/token"));
        assert_eq!(p.lock(), PathBuf::from("/tmp/x/daemon.lock"));
    }
}
