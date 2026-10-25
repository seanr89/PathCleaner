use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CleanerError {
    #[error("Target directory does not exist: {0}")]
    TargetNotFound(PathBuf),

    #[error("Target path is not a directory: {0}")]
    TargetNotDirectory(PathBuf),

    #[error("Refusing to scan protected system directory: {0}")]
    ProtectedSystemPath(PathBuf),

    #[error("Invalid duration format '{0}': {1}")]
    InvalidDuration(String, String),

    #[error("Invalid exclude glob pattern '{0}': {1}")]
    InvalidGlob(String, String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
