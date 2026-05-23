//! Session schema types — see `docs/SESSION_SCHEMA.md` for the wire-format spec.
//!
//! Field declaration order in each struct **must** match the order documented
//! in the spec example (§10). serde_json emits struct fields in declaration
//! order, so this ordering is what makes byte-equal roundtrip work against
//! the canonical fixture.
//!
//! Enum variants use `#[serde(other)] Unknown` per §7 forward-compatibility
//! rules: a v0.1 reader must accept unknown variants from a future writer
//! without rejecting the whole file.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Wire-format version. Bump per §7 semver rules.
pub const SCHEMA_VERSION: &str = "0.1.0";

// ---------------------------------------------------------------------------
// Top level
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFile {
    pub schema_version: String,
    pub session: SessionMeta,
    pub actions: Vec<Action>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub stats: Option<Stats>,
}

// ---------------------------------------------------------------------------
// Session metadata
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: Ulid,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub started_mono_ns: u64,
    pub app: AppInfo,
    pub server: ServerInfo,
    pub client: ClientInfo,
    pub sandbox: SandboxConfig,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub redactions: Option<Redactions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub build: String,
    pub os: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub transport: String,
    pub command: Vec<String>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub mode: String,
    pub fs_allow: Vec<String>,
    pub fs_deny: Vec<String>,
    pub net_allow: Vec<String>,
    pub net_default: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Redactions {
    pub paths: Vec<String>,
    pub policy: String,
}

// ---------------------------------------------------------------------------
// Action record
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: Ulid,
    pub seq: u64,
    pub parent_id: Option<Ulid>,
    pub cause_id: Option<Ulid>,
    pub ts_wall: DateTime<Utc>,
    pub ts_mono_ns: u64,
    pub duration_ns: Option<u64>,

    pub kind: Kind,
    pub actor: Actor,
    pub tool: Option<String>,
    pub args: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub outcome: Outcome,
    pub error: Option<ActionError>,
    pub decision: Option<SandboxDecision>,

    pub payload_hash: String,
    pub payload_truncated: bool,
    pub payload_size_bytes: u64,

    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    ToolCall,
    ResourceRead,
    ResourceList,
    PromptGet,
    Completion,
    Notification,
    SandboxDecision,
    SessionEvent,
    /// Forward-compatibility sink — see §7.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    Agent,
    User,
    System,
    Sandbox,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Ok,
    Error,
    Denied,
    Timeout,
    Cancelled,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionError {
    pub code: String,
    pub message: String,
    pub source: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxDecision {
    pub verdict: String,
    pub rule_id: String,
    pub reason: String,
    pub mode: String,
    pub prompted: bool,
    pub prompt_resolution: Option<String>,
}

// ---------------------------------------------------------------------------
// Stats (computed at export / recomputed by reader)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Stats {
    pub total_actions: u64,
    pub by_outcome: std::collections::BTreeMap<String, u64>,
    pub by_kind: std::collections::BTreeMap<String, u64>,
    pub denied_count: u64,
    pub error_count: u64,
    pub duration_ms: u64,
    pub avg_action_ms: f64,
    pub p95_action_ms: f64,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

// ---------------------------------------------------------------------------
// NDJSON record envelope (§2.2)
// ---------------------------------------------------------------------------

/// One line of an NDJSON session file.
///
/// The discriminator `type` field is what makes the format streamable: a reader
/// can parse one line at a time and dispatch on `type` without buffering the
/// whole file. Order MUST be `meta` → `action`* → `end`, but the reader
/// tolerates a missing `end` (truncated session).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NdjsonRecord {
    Meta {
        schema_version: String,
        session: SessionMeta,
    },
    Action {
        action: Action,
    },
    End {
        ended_at: DateTime<Utc>,
        stats: Stats,
    },
}
