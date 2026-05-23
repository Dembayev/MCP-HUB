//! Database row models that double as IPC payloads.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Stdio,
    Sse,
    Http,
}

impl Transport {
    pub fn as_str(&self) -> &'static str {
        match self {
            Transport::Stdio => "stdio",
            Transport::Sse => "sse",
            Transport::Http => "http",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "sse" => Transport::Sse,
            "http" => Transport::Http,
            _ => Transport::Stdio,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    Stopped,
    Starting,
    Running,
    Crashed,
}

impl ServerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ServerStatus::Stopped => "stopped",
            ServerStatus::Starting => "starting",
            ServerStatus::Running => "running",
            ServerStatus::Crashed => "crashed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "starting" => ServerStatus::Starting,
            "running" => ServerStatus::Running,
            "crashed" => ServerStatus::Crashed,
            _ => ServerStatus::Stopped,
        }
    }
}

/// Serialized form sent to the React frontend. Fields use camelCase to match
/// JS conventions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub id: String,
    pub name: String,
    pub description: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub transport: Transport,
    pub status: ServerStatus,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: Option<String>,
    pub source: Option<String>,
    pub icon_url: Option<String>,
}

/// Payload accepted by `install_server`. The id is generated server-side.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallServerRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_transport")]
    pub transport: Transport,
    pub version: Option<String>,
    pub source: Option<String>,
    pub icon_url: Option<String>,
    /// Permissions the user consented to in the install dialog. Stored
    /// with `granted = true` so the sandbox layer can enforce them.
    #[serde(default)]
    pub permissions: Vec<crate::db::permissions::RequestedPermission>,
}

fn default_transport() -> Transport {
    Transport::Stdio
}
