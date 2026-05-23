//! Programmatic construction of a small "demo trace" for UI seeding.
//!
//! This exists so the Timeline tab is testable BEFORE the proxy is
//! instrumented (step 4). The shape mirrors the canonical fixture used in
//! `tests/fixtures/canonical-session.ndjson` — start → allowed read →
//! denied `~/.ssh` write — but with fresh ULIDs and timestamps so multiple
//! seeds don't collide.

use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use ulid::Ulid;

use super::types::{
    Action, ActionError, Actor, AppInfo, ClientInfo, Kind, Outcome, SandboxConfig,
    SandboxDecision, ServerInfo, SessionFile, SessionMeta, Stats, SCHEMA_VERSION,
};
use super::{hash, PartialAction};

/// Build a fresh demo session ready to be written to disk. The session has
/// the same narrative arc as the canonical fixture but with `Utc::now()`-based
/// timestamps and a new ULID — calling this twice produces two distinct
/// session files.
pub fn make_demo_session() -> SessionFile {
    let session_id = Ulid::new();
    let now = Utc::now();
    let started_at = now;
    let ended_at = now + Duration::milliseconds(3_900);

    let cause = Ulid::new(); // synthetic "model turn" cause id

    let action0 = PartialAction::lifecycle(0, "start", started_at, 0).complete(None, 0, None);

    let action1 = build_allowed_read(1, started_at, cause);
    let action2 = build_denied_write(2, started_at, cause);

    let actions = vec![action0, action1, action2];
    let stats = compute_demo_stats(&actions);

    SessionFile {
        schema_version: SCHEMA_VERSION.to_string(),
        session: SessionMeta {
            id: session_id,
            started_at,
            ended_at: Some(ended_at),
            started_mono_ns: 0,
            app: AppInfo {
                name: "mcp-hub".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                build: option_env!("VERGEN_GIT_SHA").unwrap_or("dev").into(),
                os: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            },
            server: ServerInfo {
                id: "demo.filesystem".into(),
                name: "Filesystem (demo)".into(),
                version: "0.6.2".into(),
                transport: "stdio".into(),
                command: vec![
                    "npx".into(),
                    "@modelcontextprotocol/server-filesystem".into(),
                    "/Users/demo/Projects".into(),
                ],
                capabilities: vec!["tools".into(), "resources".into()],
            },
            client: ClientInfo {
                name: "claude-desktop".into(),
                version: "0.9.4".into(),
            },
            sandbox: SandboxConfig {
                mode: "enforce".into(),
                fs_allow: vec!["/Users/demo/Projects".into()],
                fs_deny: vec!["~/.ssh".into(), "~/.aws".into()],
                net_allow: vec![],
                net_default: "deny".into(),
            },
            redactions: None,
        },
        actions,
        stats: Some(stats),
    }
}

fn build_allowed_read(seq: u64, anchor: DateTime<Utc>, cause: Ulid) -> Action {
    let args = json!({"path": "/Users/demo/Projects/site/package.json"});
    let result = json!({
        "content": "{\"name\":\"site\",\"version\":\"0.1.0\"}",
        "mime": "application/json"
    });
    let ts_wall = anchor + Duration::milliseconds(2_275);
    let ts_mono_ns = 2_275_000_000;
    let payload_hash = hash::payload_hash(Some(&args), Some(&result));
    Action {
        id: Ulid::new(),
        seq,
        parent_id: None,
        cause_id: Some(cause),
        ts_wall,
        ts_mono_ns,
        duration_ns: Some(18_324_000),
        kind: Kind::ToolCall,
        actor: Actor::Agent,
        tool: Some("read_file".into()),
        args: Some(args),
        result: Some(result),
        outcome: Outcome::Ok,
        error: None,
        decision: Some(SandboxDecision {
            verdict: "allow".into(),
            rule_id: "fs.allow.projects".into(),
            reason: "Path inside fs_allow".into(),
            mode: "enforce".into(),
            prompted: false,
            prompt_resolution: None,
        }),
        payload_hash,
        payload_truncated: false,
        payload_size_bytes: 2_148,
        tags: vec!["fs".into(), "read".into()],
    }
}

fn build_denied_write(seq: u64, anchor: DateTime<Utc>, cause: Ulid) -> Action {
    let args = json!({
        "content": "Host evil\n  HostName 10.0.0.1",
        "path": "/Users/demo/.ssh/config"
    });
    let ts_wall = anchor + Duration::milliseconds(2_971);
    let ts_mono_ns = 2_971_000_000;
    let payload_hash = hash::payload_hash(Some(&args), None);
    Action {
        id: Ulid::new(),
        seq,
        parent_id: None,
        cause_id: Some(cause),
        ts_wall,
        ts_mono_ns,
        duration_ns: Some(4_102_000),
        kind: Kind::ToolCall,
        actor: Actor::Agent,
        tool: Some("write_file".into()),
        args: Some(args),
        result: None,
        outcome: Outcome::Denied,
        error: Some(ActionError {
            code: "SANDBOX_DENY".into(),
            message: "Write to ~/.ssh/config blocked by policy".into(),
            source: "sandbox".into(),
            data: None,
        }),
        decision: Some(SandboxDecision {
            verdict: "deny".into(),
            rule_id: "fs.write".into(),
            reason: "User reviewed approval prompt and chose Deny".into(),
            mode: "enforce".into(),
            prompted: true,
            prompt_resolution: Some("deny".into()),
        }),
        payload_hash,
        payload_truncated: false,
        payload_size_bytes: 78,
        tags: vec!["fs".into(), "write".into(), "denied".into(), "prompted".into()],
    }
}

fn compute_demo_stats(actions: &[Action]) -> Stats {
    use std::collections::BTreeMap;
    let mut by_outcome: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_kind: BTreeMap<String, u64> = BTreeMap::new();
    let mut denied = 0u64;
    for a in actions {
        let out = serde_json::to_value(a.outcome)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".into());
        let kind = serde_json::to_value(a.kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".into());
        *by_outcome.entry(out.clone()).or_default() += 1;
        *by_kind.entry(kind).or_default() += 1;
        if out == "denied" {
            denied += 1;
        }
    }
    Stats {
        total_actions: actions.len() as u64,
        by_outcome,
        by_kind,
        denied_count: denied,
        error_count: 0,
        duration_ms: 3_900,
        avg_action_ms: 7.5,
        p95_action_ms: 18.3,
        bytes_in: 0,
        bytes_out: 0,
    }
}
