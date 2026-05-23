//! Agent action capture.
//!
//! MCP servers speak JSON-RPC 2.0 over stdio. Whenever a server emits a
//! line on stdout that matches a known shape, we classify it into a
//! semantic [`AgentAction`] (read a file, fetch a URL, run a command, …)
//! and emit a Tauri `agent-action` event that drives the Timeline UI.
//!
//! ## Architectural honesty
//!
//! Today we only see one half of the JSON-RPC conversation — the side
//! the server emits on stdout. Tool *invocations* (`tools/call` requests)
//! travel client → server on stdin and are invisible to us. So this
//! classifier mostly fires on server-emitted notifications and on
//! responses where the tool name was echoed back, plus the Demo Mode
//! synthetic stream on the frontend.
//!
//! Becoming a full MCP proxy (sitting between the client and the server)
//! is the planned upgrade — once we proxy, the same Timeline UI fills
//! up with real traffic without any frontend changes.

use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActionKind {
    FsRead,
    FsWrite,
    BrowserOpen,
    HttpFetch,
    TerminalExec,
    MemoryStore,
    Search,
    ToolCall,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ActionStatus {
    /// Request seen by the proxy, not yet completed.
    Pending,
    /// Server returned a successful response.
    Success,
    /// Blocked by policy (sandbox or proxy permission check).
    Denied,
    /// Server returned an error response.
    Failed,
    /// Client cancelled before completion.
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAction {
    pub id: String,
    pub server_id: String,
    pub kind: ActionKind,
    pub tool_name: String,
    /// Short summary of what was acted on — path / URL / command.
    pub target: Option<String>,
    /// Full JSON-RPC `params.arguments`, for the expandable card detail.
    pub params: Option<Value>,
    /// JSON-RPC request id, if any.
    pub request_id: Option<Value>,
    pub status: ActionStatus,
    /// When `status == Denied`, the reason surfaced to the user.
    pub denied_reason: Option<String>,
    /// When `status == Failed`, the server-reported error.
    pub error: Option<String>,
    /// Round-trip latency. Set on terminal-state updates.
    pub latency_ms: Option<u64>,
    pub timestamp: DateTime<Utc>,
}

/// Try to classify a single line of server output as a semantic agent action.
///
/// Returns `None` for non-JSON, JSON that isn't a JSON-RPC envelope, or
/// envelopes that don't carry a tool-call.
pub fn classify_jsonrpc(line: &str, server_id: &str) -> Option<AgentAction> {
    // Fast path: only attempt parse if the line looks like JSON. Saves the
    // serde overhead on the dozens of plain-text log lines servers emit.
    let trimmed = line.trim_start();
    if !trimmed.starts_with('{') {
        return None;
    }
    let value: Value = serde_json::from_str(trimmed).ok()?;

    // We're interested in `tools/call` envelopes (both directions emit them
    // in proxy mode) and any `notifications/*` server-pushes.
    let method = value.get("method").and_then(Value::as_str)?;

    // Currently we only build cards for tools/call. Everything else (init,
    // ping, list, …) is plumbing.
    if method != "tools/call" && !method.starts_with("notifications/") {
        return None;
    }

    let params = value.get("params");
    let tool_name = params
        .and_then(|p| p.get("name"))
        .and_then(Value::as_str)
        .unwrap_or(method)
        .to_string();
    let args = params.and_then(|p| p.get("arguments")).cloned();
    let request_id = value.get("id").cloned();

    let (kind, target) = classify_tool(&tool_name, args.as_ref());

    Some(AgentAction {
        id: Uuid::new_v4().to_string(),
        server_id: server_id.to_string(),
        kind,
        tool_name,
        target,
        params: args,
        request_id,
        status: ActionStatus::Success,
        denied_reason: None,
        error: None,
        latency_ms: None,
        timestamp: Utc::now(),
    })
}

/// Best-effort detection of sandbox/permission denials in a stderr line.
/// Many sandboxes write a distinctive marker (macOS sandbox-exec emits
/// "Sandbox: <prog>(<pid>) deny <op>" via the kernel log; the child also
/// frequently sees `Operation not permitted` when a syscall is blocked).
///
/// When we see one of these markers we synthesize a `Denied` AgentAction
/// so the Timeline surfaces the violation in the same UI as a successful
/// action — "Timeline = single source of truth".
pub fn classify_sandbox_denial(line: &str, server_id: &str) -> Option<AgentAction> {
    let lower = line.to_ascii_lowercase();
    let (kind, reason) = if lower.contains("sandbox") && lower.contains("deny") {
        (ActionKind::Other, "Blocked by sandbox policy")
    } else if lower.contains("operation not permitted") {
        (ActionKind::Other, "Operation not permitted (sandbox)")
    } else if lower.contains("eperm") {
        (ActionKind::Other, "EPERM: blocked")
    } else if lower.contains("network is unreachable")
        || (lower.contains("connect") && lower.contains("refused") && lower.contains("sandbox"))
    {
        (ActionKind::HttpFetch, "Network blocked by sandbox")
    } else {
        return None;
    };

    Some(AgentAction {
        id: Uuid::new_v4().to_string(),
        server_id: server_id.to_string(),
        kind,
        tool_name: "sandbox".to_string(),
        target: Some(truncate_for_target(line, 140)),
        params: None,
        request_id: None,
        status: ActionStatus::Denied,
        denied_reason: Some(reason.to_string()),
        error: None,
        latency_ms: None,
        timestamp: Utc::now(),
    })
}

fn truncate_for_target(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    }
}

/// Map a tool name + arguments to an [`ActionKind`] and a target string for
/// the card subtitle. The heuristic favors well-known MCP server tools but
/// degrades gracefully to `ToolCall` for anything unrecognized.
fn classify_tool(name: &str, args: Option<&Value>) -> (ActionKind, Option<String>) {
    let n = name.to_lowercase();

    if matches_any(&n, &["read_file", "read_text_file", "read_multiple_files",
                         "list_directory", "directory_tree", "get_file_info"])
        || (n.contains("read") && (n.contains("file") || n.contains("dir")))
    {
        return (ActionKind::FsRead, pick_str(args, &["path", "filepath"]));
    }

    if matches_any(&n, &["write_file", "edit_file", "create_directory", "move_file"])
        || n.starts_with("write_")
        || n.starts_with("edit_")
    {
        return (
            ActionKind::FsWrite,
            pick_str(args, &["path", "dst_path", "destination", "filepath"]),
        );
    }

    if n.contains("puppeteer")
        || n.contains("browser")
        || n.contains("screenshot")
        || n.contains("navigate")
    {
        return (ActionKind::BrowserOpen, pick_str(args, &["url"]));
    }

    if n == "fetch" || n.starts_with("http_") || n.contains("get_url") {
        return (ActionKind::HttpFetch, pick_str(args, &["url"]));
    }

    if n.contains("execute") || n.contains("shell") || n == "bash" || n == "run_command" {
        return (ActionKind::TerminalExec, pick_str(args, &["command", "cmd"]));
    }

    if n.contains("memory") || n.contains("entities") || n.contains("observations") {
        let target = args
            .and_then(|a| a.get("entities"))
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
            .and_then(|e| e.get("name"))
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        return (ActionKind::MemoryStore, target);
    }

    if n.contains("search") || n == "google" || n == "brave_web_search" {
        return (ActionKind::Search, pick_str(args, &["query", "q"]));
    }

    (ActionKind::ToolCall, None)
}

fn matches_any(haystack: &str, options: &[&str]) -> bool {
    options.iter().any(|o| haystack == *o)
}

fn pick_str(args: Option<&Value>, keys: &[&str]) -> Option<String> {
    let args = args?;
    for key in keys {
        if let Some(v) = args.get(*key).and_then(Value::as_str) {
            return Some(v.to_string());
        }
    }
    None
}

/// Per-server ring buffer of recent actions. Same shape as [`LogSink`] —
/// the two are decoupled because a single log line might produce zero or
/// one actions, and we want the action stream to be replayable on its own.
pub struct ActionSink {
    buffers: Mutex<HashMap<String, VecDeque<AgentAction>>>,
    capacity_per_server: usize,
}

impl ActionSink {
    pub fn new(capacity_per_server: usize) -> Self {
        Self {
            buffers: Mutex::new(HashMap::new()),
            capacity_per_server,
        }
    }

    pub fn push(&self, action: &AgentAction) {
        let mut map = self.buffers.lock();
        let q = map
            .entry(action.server_id.clone())
            .or_insert_with(|| VecDeque::with_capacity(self.capacity_per_server));
        if q.len() >= self.capacity_per_server {
            q.pop_front();
        }
        q.push_back(action.clone());
    }

    /// Update an existing action by id if present, else append. Used by the
    /// proxy when an in-flight request transitions Pending → Success/Failed.
    pub fn push_or_update(&self, action: &AgentAction) {
        let mut map = self.buffers.lock();
        let q = map
            .entry(action.server_id.clone())
            .or_insert_with(|| VecDeque::with_capacity(self.capacity_per_server));
        if let Some(pos) = q.iter().position(|a| a.id == action.id) {
            q[pos] = action.clone();
        } else {
            if q.len() >= self.capacity_per_server {
                q.pop_front();
            }
            q.push_back(action.clone());
        }
    }

    pub fn snapshot(&self, server_id: &str, limit: usize) -> Vec<AgentAction> {
        let map = self.buffers.lock();
        match map.get(server_id) {
            Some(q) => {
                let start = q.len().saturating_sub(limit);
                q.iter().skip(start).cloned().collect()
            }
            None => Vec::new(),
        }
    }

    pub fn clear(&self, server_id: &str) {
        self.buffers.lock().remove(server_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_read_file() {
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"read_file","arguments":{"path":"/etc/hosts"}}}"#;
        let a = classify_jsonrpc(line, "srv").expect("classified");
        assert!(matches!(a.kind, ActionKind::FsRead));
        assert_eq!(a.tool_name, "read_file");
        assert_eq!(a.target.as_deref(), Some("/etc/hosts"));
    }

    #[test]
    fn classifies_fetch_url() {
        let line = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"fetch","arguments":{"url":"https://api.github.com"}}}"#;
        let a = classify_jsonrpc(line, "srv").expect("classified");
        assert!(matches!(a.kind, ActionKind::HttpFetch));
        assert_eq!(a.target.as_deref(), Some("https://api.github.com"));
    }

    #[test]
    fn ignores_non_jsonrpc() {
        assert!(classify_jsonrpc("plain log line", "srv").is_none());
        assert!(classify_jsonrpc("", "srv").is_none());
        assert!(classify_jsonrpc(r#"{"not":"jsonrpc"}"#, "srv").is_none());
    }

    #[test]
    fn ignores_uninteresting_methods() {
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        assert!(classify_jsonrpc(line, "srv").is_none());
    }

    #[test]
    fn unknown_tool_falls_back_to_tool_call() {
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"weird_thing","arguments":{}}}"#;
        let a = classify_jsonrpc(line, "srv").expect("classified");
        assert!(matches!(a.kind, ActionKind::ToolCall));
        assert_eq!(a.target, None);
    }
}
