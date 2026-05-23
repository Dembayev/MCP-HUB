//! Permission model surfaced to the user.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionScope {
    /// Read files matching a path glob.
    FsRead,
    /// Write files matching a path glob.
    FsWrite,
    /// Outbound network access to a host pattern.
    NetOutbound,
    /// Read environment variables.
    EnvRead,
    /// Execute subprocesses.
    Exec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    pub id: i64,
    pub server_id: String,
    pub scope: PermissionScope,
    /// Glob/pattern for the resource (path, host, etc).
    pub target: Option<String>,
    pub granted: bool,
    pub granted_at: Option<DateTime<Utc>>,
}
