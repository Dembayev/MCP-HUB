//! Runtime approval flow — the third verb of "See → Replay → Approve".
//!
//! When an MCP request comes in and the required permission scope isn't
//! pre-granted, the proxy creates an [`ApprovalRequest`], registers a oneshot
//! channel here, emits a Tauri event to the UI, and **awaits the user's
//! decision** before forwarding (or synthesizing a denial). The UI surfaces
//! a permission popup; the user clicks Allow Once / Always Allow / Deny;
//! the resulting [`ApprovalDecision`] is delivered back through the oneshot.
//!
//! Per `mcp_hub_launch_guardrails`: this is the **approval loop**, not a
//! policy engine. No risk heuristics beyond the static [`risk_for_kind`]
//! lookup. No trust scores. No AI risk analysis. Human in the loop.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::mcp::ActionKind;

// ---------------------------------------------------------------------------
// Payloads
// ---------------------------------------------------------------------------

/// Sent to the frontend via `approval-requested` Tauri event. Carries
/// everything the popup needs to render and route the decision back.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    pub id: String,
    pub server_id: String,
    pub server_name: String,
    pub tool: String,
    pub kind: ActionKind,
    pub target: Option<String>,
    /// Scope token that "Always Allow" would grant. Surfaced to the UI so
    /// the popup can say "MCP Hub will not ask again for fs.write".
    pub scope: String,
    /// Static risk classification — not a heuristic engine, just three
    /// buckets so the UI can color the popup proportionally.
    pub risk: &'static str,
    pub requested_at: DateTime<Utc>,
}

/// User's choice. Delivered back through the oneshot.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Grant for this single request. Don't persist.
    AllowOnce,
    /// Grant for this request AND persist the scope so MCP Hub doesn't
    /// prompt again for the same scope on this server.
    AllowSession,
    /// Refuse this request. The proxy synthesizes a JSON-RPC error response
    /// to the client.
    Deny,
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// In-memory registry of pending approvals. One per `AppState`.
///
/// The proxy registers a sender keyed by approval id and awaits the
/// receiver. The Tauri `resolve_approval` command looks up the sender by
/// id and delivers the decision. If the UI process disconnects or the
/// modal is closed without resolution, the sender is dropped by [`reap`]
/// (called on shutdown) and the awaiter receives `Err(RecvError)` — which
/// the proxy treats as a fail-safe `Deny`.
pub struct ApprovalRegistry {
    pending: Mutex<HashMap<String, oneshot::Sender<ApprovalDecision>>>,
}

impl ApprovalRegistry {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
        }
    }

    /// Register a pending approval. Returns nothing — caller owns the receiver
    /// half of the oneshot and awaits it themselves.
    pub fn register(&self, id: String, sender: oneshot::Sender<ApprovalDecision>) {
        self.pending.lock().insert(id, sender);
    }

    /// Resolve a pending approval by id. Returns `true` if the approval was
    /// found and the decision was delivered; `false` otherwise (already
    /// resolved, expired, or never registered).
    pub fn resolve(&self, id: &str, decision: ApprovalDecision) -> bool {
        let sender = self.pending.lock().remove(id);
        match sender {
            Some(tx) => tx.send(decision).is_ok(),
            None => false,
        }
    }

    /// Number of currently-pending approvals. Useful for UI badges and tests.
    pub fn pending_count(&self) -> usize {
        self.pending.lock().len()
    }
}

impl Default for ApprovalRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Static risk classification
// ---------------------------------------------------------------------------

/// Bucket the request into "low" / "medium" / "high" purely by static rules.
/// This is NOT a heuristic engine — it's a lookup table the UI uses to color
/// the popup. The user always makes the call.
pub fn risk_for_kind(kind: ActionKind, target: Option<&str>) -> &'static str {
    match kind {
        ActionKind::FsWrite => {
            if target.is_some_and(|t| is_sensitive_fs_path(t)) {
                "high"
            } else {
                "medium"
            }
        }
        ActionKind::TerminalExec => "high",
        ActionKind::FsRead => {
            if target.is_some_and(|t| is_sensitive_fs_path(t)) {
                "medium"
            } else {
                "low"
            }
        }
        ActionKind::HttpFetch | ActionKind::BrowserOpen => "medium",
        ActionKind::Search | ActionKind::MemoryStore => "low",
        ActionKind::ToolCall | ActionKind::Other => "low",
    }
}

/// Cheap path check: paths under `~/.ssh`, `~/.aws`, `/etc/`, etc. are
/// considered sensitive regardless of action kind.
fn is_sensitive_fs_path(target: &str) -> bool {
    let lowered = target.to_lowercase();
    SENSITIVE_PATH_FRAGMENTS
        .iter()
        .any(|frag| lowered.contains(frag))
}

const SENSITIVE_PATH_FRAGMENTS: &[&str] = &[
    ".ssh",
    ".aws",
    ".gnupg",
    "/etc/",
    "/var/root",
    "/system/",
    "/keychain",
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_buckets_sensible() {
        assert_eq!(risk_for_kind(ActionKind::FsRead, Some("/tmp/x")), "low");
        assert_eq!(
            risk_for_kind(ActionKind::FsRead, Some("/Users/x/.ssh/config")),
            "medium",
        );
        assert_eq!(risk_for_kind(ActionKind::FsWrite, Some("/tmp/x")), "medium");
        assert_eq!(
            risk_for_kind(ActionKind::FsWrite, Some("/Users/x/.ssh/config")),
            "high",
        );
        assert_eq!(risk_for_kind(ActionKind::TerminalExec, None), "high");
        assert_eq!(risk_for_kind(ActionKind::Search, None), "low");
    }

    #[tokio::test]
    async fn registry_round_trips_a_decision() {
        let reg = ApprovalRegistry::new();
        let (tx, rx) = oneshot::channel();
        reg.register("test-1".into(), tx);
        assert_eq!(reg.pending_count(), 1);

        // Simulate the resolve_approval command.
        let delivered = reg.resolve("test-1", ApprovalDecision::AllowOnce);
        assert!(delivered);
        assert_eq!(reg.pending_count(), 0);

        let received = rx.await.expect("oneshot");
        assert!(matches!(received, ApprovalDecision::AllowOnce));
    }

    #[test]
    fn resolve_unknown_id_returns_false() {
        let reg = ApprovalRegistry::new();
        assert!(!reg.resolve("nope", ApprovalDecision::Deny));
    }
}
