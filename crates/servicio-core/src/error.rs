use thiserror::Error;

/// All fallible operations in servicio-core return this error.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("working directory does not exist: {0}")]
    MissingWorkingDir(String),

    #[error("failed to spawn process: {0}")]
    Spawn(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid state transition: {from} -> {to}")]
    BadTransition { from: String, to: String },
}
