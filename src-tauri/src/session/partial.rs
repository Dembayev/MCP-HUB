//! Pending-request state and completion folds for the trace pipeline.
//!
//! Per `mcp_hub_launch_guardrails` (memory): `PartialAction` is **immutable**;
//! it is consumed by a pure fold function at the moment of completion. There
//! is deliberately no mid-flight mutation — that pattern keeps the
//! request→response lifecycle observable from exactly one place (the fold
//! call site) and makes the completion logic trivially unit-testable.
//!
//! Typical lifecycle:
//!
//! ```ignore
//! // 1. Request arrives at proxy — build the partial.
//! let partial = PartialAction::tool_call(seq, "read_file".into(),
//!                                        Some(json!({"path": "/tmp"})), cause_id);
//!
//! // 2. Store in pending map keyed by JSON-RPC id (HashMap<RpcId, PartialAction>).
//! pending.insert(rpc_id, partial);
//!
//! // 3. Response arrives — remove from map and fold.
//! let partial = pending.remove(&rpc_id).unwrap();
//! let action = partial.complete(result_value, duration_ns, decision);
//!
//! // 4. Send to writer task.
//! appender.append(action).await?;
//! ```

use chrono::{DateTime, Utc};
use serde_json::Value;
use ulid::Ulid;

use super::hash;
use super::types::{Action, ActionError, Actor, Kind, Outcome, SandboxDecision};

/// Captured at request-arrival time. All fields needed to identify the action
/// and reconstruct request-side state, frozen at the moment of capture.
///
/// `parent_id` and `cause_id` are populated by the caller from the surrounding
/// trace context (e.g. the session-start action ID for `cause_id`). For
/// top-level events both are `None`.
#[derive(Debug, Clone)]
pub struct PartialAction {
    pub id: Ulid,
    pub seq: u64,
    pub parent_id: Option<Ulid>,
    pub cause_id: Option<Ulid>,
    pub ts_wall: DateTime<Utc>,
    pub ts_mono_ns: u64,
    pub kind: Kind,
    pub actor: Actor,
    pub tool: Option<String>,
    pub args: Option<Value>,
    pub tags: Vec<String>,
}

impl PartialAction {
    /// Construct a partial for a `tools/call` request.
    ///
    /// Time is captured by the caller (typically `Utc::now()` + a monotonic
    /// offset from the session anchor) — we do NOT call `now()` inside the
    /// fold so tests can drive deterministic timestamps.
    pub fn tool_call(
        seq: u64,
        tool: String,
        args: Option<Value>,
        ts_wall: DateTime<Utc>,
        ts_mono_ns: u64,
        cause_id: Option<Ulid>,
    ) -> Self {
        Self {
            id: Ulid::new(),
            seq,
            parent_id: None,
            cause_id,
            ts_wall,
            ts_mono_ns,
            kind: Kind::ToolCall,
            actor: Actor::Agent,
            tool: Some(tool),
            args,
            tags: Vec::new(),
        }
    }

    /// Construct a partial for an MCP lifecycle event (session start/end,
    /// disconnect). Synthesised by MCP Hub itself, not the agent.
    pub fn lifecycle(
        seq: u64,
        event: &str,
        ts_wall: DateTime<Utc>,
        ts_mono_ns: u64,
    ) -> Self {
        Self {
            id: Ulid::new(),
            seq,
            parent_id: None,
            cause_id: None,
            ts_wall,
            ts_mono_ns,
            kind: Kind::SessionEvent,
            actor: Actor::System,
            tool: None,
            args: Some(serde_json::json!({"event": event})),
            tags: vec!["lifecycle".into()],
        }
    }

    /// Append a tag. Returned `Self` keeps the builder shape ergonomic without
    /// exposing mutation — each call produces a new partial that consumes the
    /// previous one.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Override `parent_id` (e.g. linking a sandbox-decision sub-action to its
    /// originating `tool_call`).
    pub fn with_parent(mut self, parent: Ulid) -> Self {
        self.parent_id = Some(parent);
        self
    }

    // -----------------------------------------------------------------------
    // Completion folds — each consumes `self` and yields a final `Action`.
    // The `payload_hash` is computed over the original args+result here; no
    // earlier caller needs to compute it.
    // -----------------------------------------------------------------------

    /// Complete on a successful response.
    ///
    /// `duration_ns` is the elapsed monotonic time from request to response,
    /// measured by the caller. `decision` is `Some` only when a sandbox
    /// allow-decision was recorded inline (most common case — even allows
    /// carry a decision per spec §5.4).
    pub fn complete(
        self,
        result: Option<Value>,
        duration_ns: u64,
        decision: Option<SandboxDecision>,
    ) -> Action {
        let payload_hash = hash::payload_hash(self.args.as_ref(), result.as_ref());
        let payload_size_bytes = approx_payload_size(&self.args, result.as_ref());
        Action {
            id: self.id,
            seq: self.seq,
            parent_id: self.parent_id,
            cause_id: self.cause_id,
            ts_wall: self.ts_wall,
            ts_mono_ns: self.ts_mono_ns,
            duration_ns: Some(duration_ns),
            kind: self.kind,
            actor: self.actor,
            tool: self.tool,
            args: self.args,
            result,
            outcome: Outcome::Ok,
            error: None,
            decision,
            payload_hash,
            payload_truncated: false,
            payload_size_bytes,
            tags: self.tags,
        }
    }

    /// Complete on an error response from the underlying MCP server.
    pub fn complete_error(self, error: ActionError, duration_ns: u64) -> Action {
        let payload_hash = hash::payload_hash(self.args.as_ref(), None);
        let payload_size_bytes = approx_payload_size(&self.args, None);
        Action {
            id: self.id,
            seq: self.seq,
            parent_id: self.parent_id,
            cause_id: self.cause_id,
            ts_wall: self.ts_wall,
            ts_mono_ns: self.ts_mono_ns,
            duration_ns: Some(duration_ns),
            kind: self.kind,
            actor: self.actor,
            tool: self.tool,
            args: self.args,
            result: None,
            outcome: Outcome::Error,
            error: Some(error),
            decision: None,
            payload_hash,
            payload_truncated: false,
            payload_size_bytes,
            tags: self.tags,
        }
    }

    /// Complete on a sandbox denial — the request is short-circuited before it
    /// reaches the MCP server, so there is no `result` and `duration_ns` is
    /// optional (typically near-zero; we record what the caller measured).
    pub fn complete_denied(
        self,
        decision: SandboxDecision,
        error: ActionError,
        duration_ns: Option<u64>,
    ) -> Action {
        let payload_hash = hash::payload_hash(self.args.as_ref(), None);
        let payload_size_bytes = approx_payload_size(&self.args, None);
        let mut tags = self.tags;
        if !tags.iter().any(|t| t == "denied") {
            tags.push("denied".into());
        }
        Action {
            id: self.id,
            seq: self.seq,
            parent_id: self.parent_id,
            cause_id: self.cause_id,
            ts_wall: self.ts_wall,
            ts_mono_ns: self.ts_mono_ns,
            duration_ns,
            kind: self.kind,
            actor: self.actor,
            tool: self.tool,
            args: self.args,
            result: None,
            outcome: Outcome::Denied,
            error: Some(error),
            decision: Some(decision),
            payload_hash,
            payload_truncated: false,
            payload_size_bytes,
            tags,
        }
    }
}

/// Best-effort approximation of original payload size. Used for `payload_size_bytes`.
/// We serialize compactly and take the byte length — close enough for the UI
/// "total bytes traced" stat without paying for double-canonicalization.
fn approx_payload_size(args: &Option<Value>, result: Option<&Value>) -> u64 {
    let mut total = 0u64;
    if let Some(a) = args {
        total += serde_json::to_string(a).map(|s| s.len() as u64).unwrap_or(0);
    }
    if let Some(r) = result {
        total += serde_json::to_string(r).map(|s| s.len() as u64).unwrap_or(0);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 23, 14, 0, 0).unwrap()
    }

    #[test]
    fn tool_call_complete_ok_produces_full_action() {
        let p = PartialAction::tool_call(
            7,
            "read_file".into(),
            Some(json!({"path": "/tmp"})),
            ts(),
            1_000_000,
            None,
        );
        let id = p.id;
        let action = p.complete(Some(json!({"content": "..."})), 5_000_000, None);

        assert_eq!(action.id, id);
        assert_eq!(action.seq, 7);
        assert_eq!(action.outcome, Outcome::Ok);
        assert_eq!(action.kind, Kind::ToolCall);
        assert_eq!(action.actor, Actor::Agent);
        assert_eq!(action.duration_ns, Some(5_000_000));
        assert!(action.error.is_none());
        assert!(action.payload_hash.starts_with("sha256:"));
        assert!(action.payload_size_bytes > 0);
    }

    #[test]
    fn complete_error_carries_error_and_no_result() {
        let p = PartialAction::tool_call(
            1,
            "fetch".into(),
            Some(json!({"url": "https://x"})),
            ts(),
            0,
            None,
        );
        let err = ActionError {
            code: "EBADURL".into(),
            message: "bad url".into(),
            source: "tool".into(),
            data: None,
        };
        let action = p.complete_error(err, 1_000);

        assert_eq!(action.outcome, Outcome::Error);
        assert!(action.result.is_none());
        assert_eq!(action.error.as_ref().unwrap().code, "EBADURL");
        // No result → partial hash.
        assert!(action.payload_hash.starts_with("sha256-partial:"));
    }

    #[test]
    fn complete_denied_tags_action_and_carries_decision() {
        let p = PartialAction::tool_call(
            2,
            "write_file".into(),
            Some(json!({"path": "/etc/passwd"})),
            ts(),
            0,
            None,
        );
        let decision = SandboxDecision {
            verdict: "deny".into(),
            rule_id: "fs.deny.system".into(),
            reason: "system path".into(),
            mode: "enforce".into(),
            prompted: false,
            prompt_resolution: None,
        };
        let err = ActionError {
            code: "SANDBOX_DENY".into(),
            message: "blocked".into(),
            source: "sandbox".into(),
            data: None,
        };
        let action = p.complete_denied(decision, err, Some(500));

        assert_eq!(action.outcome, Outcome::Denied);
        assert!(action.tags.contains(&"denied".to_string()));
        assert_eq!(action.decision.as_ref().unwrap().rule_id, "fs.deny.system");
        assert!(action.payload_hash.starts_with("sha256-partial:"));
    }

    #[test]
    fn lifecycle_event_is_system_actor_with_lifecycle_tag() {
        let p = PartialAction::lifecycle(0, "start", ts(), 0);
        assert_eq!(p.actor, Actor::System);
        assert_eq!(p.kind, Kind::SessionEvent);
        assert!(p.tags.contains(&"lifecycle".to_string()));

        let action = p.complete(None, 0, None);
        assert_eq!(action.outcome, Outcome::Ok);
        // Per spec §5.5: any action without a `result` side gets the
        // `sha256-partial:` prefix — including semantically-void cases like
        // lifecycle events. The prefix means "hash covers args only", not
        // "action is incomplete". Documented as a wording cleanup candidate
        // for v0.2 spec revision (see SESSION_SCHEMA.md §12 follow-up).
        assert!(action.payload_hash.starts_with("sha256-partial:"));
    }

    #[test]
    fn with_tag_and_with_parent_are_chainable_and_immutable() {
        let p = PartialAction::tool_call(0, "x".into(), None, ts(), 0, None)
            .with_tag("fs")
            .with_tag("read")
            .with_parent(Ulid::new());
        assert_eq!(p.tags, vec!["fs", "read"]);
        assert!(p.parent_id.is_some());
    }
}
