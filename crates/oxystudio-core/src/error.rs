use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Zip error: {0}")]
    Zip(String),

    #[error("Version parse error: {0}")]
    VersionParse(String),

    #[error("No release found for {0}")]
    NoReleaseFound(String),

    #[error("No compatible release asset found for {app} on {os}-{arch}")]
    NoCompatibleAsset {
        app: String,
        os: String,
        arch: String,
    },

    #[error(
        "Checksum verification failed for {file}: expected {expected}, calculated {calculated}"
    )]
    ChecksumMismatch {
        file: String,
        expected: String,
        calculated: String,
    },

    #[error("Checksum file not found or corrupted for {0}")]
    ChecksumNotFound(String),

    #[error("Executable not found in extracted directory: {0}")]
    ExecutableNotFound(PathBuf),

    #[error("Launch failed: {0}")]
    Launch(String),

    #[error("App not installed: {0}")]
    NotInstalled(String),
}

pub type Result<T> = std::result::Result<T, Error>;
