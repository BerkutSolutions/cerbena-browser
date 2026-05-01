use std::{io, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("profile validation failed: {0}")]
    Validation(String),
    #[error("profile not found: {0}")]
    NotFound(String),
    #[error("profile already exists: {0}")]
    AlreadyExists(String),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("path traversal attempt blocked: {0}")]
    InvalidPath(PathBuf),
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("profile is locked: {0}")]
    Locked(String),
    #[error("unlock attempts exceeded for profile: {0}")]
    UnlockAttemptsExceeded(String),
    #[error("update conflict: {0}")]
    Conflict(String),
}
