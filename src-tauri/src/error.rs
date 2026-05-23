//! Unified error type for MCP Hub.

use serde::{Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("server not found: {0}")]
    ServerNotFound(String),

    #[error("server already exists: {0}")]
    ServerAlreadyExists(String),

    #[error("invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("process error: {0}")]
    Process(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;

// Tauri commands must return a serializable error. We render AppError as its
// Display string so the frontend gets a clean human-readable message.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
