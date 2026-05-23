//! Integration tests for the session tracing layer.
//!
//! Snapshot-testing pattern (per docs/SESSION_SCHEMA.md §10 / §13):
//!
//! 1. [`canonical_session`] constructs the §10 canonical demo trace
//!    programmatically from typed values.
//! 2. The fixture files on disk are the **serialized snapshot** of that
//!    typed value, written out once and committed.
//! 3. Tests verify byte-equal round-trip: parse → reserialize → bytes match.
//!
//! To regenerate the snapshots after an intentional schema change:
//!
//!     UPDATE_FIXTURES=1 cargo test --test session_roundtrip
//!
//! This is the discipline that lets us keep the schema honestly frozen: any
//! accidental drift in the writer breaks the byte-equal assertion immediately.

use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use serde_json::json;
use ulid::Ulid;

use mcp_hub_lib::session::*;

// ---------------------------------------------------------------------------
// Canonical session — the §10 demo trace as typed values.
// ---------------------------------------------------------------------------

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn ulid(s: &str) -> Ulid {
    Ulid::from_string(s).expect("test ULID must be valid Crockford Base32")
}

fn canonical_session() -> SessionFile {
    // Three actions:
    //   0: session start (lifecycle)
    //   1: allowed read_file
    //   2: denied write to ~/.ssh/config — the demo CLIMAX
    let session_start = ulid("01HXP4M9C1AAAAAAAAAAAAAAAA");
    let action_read = ulid("01HXP4M9C2BBBBBBBBBBBBBBBB");
    let action_denied = ulid("01HXP4M9C3CCCCCCCCCCCCCCCC");

    let actions = vec![
        Action {
            id: session_start,
            seq: 0,
            parent_id: None,
            cause_id: None,
            ts_wall: Utc.with_ymd_and_hms(2026, 5, 23, 14, 2, 11).unwrap()
                + chrono::Duration::milliseconds(137),
            ts_mono_ns: 0,
            duration_ns: None,
            kind: Kind::SessionEvent,
            actor: Actor::System,
            tool: None,
            args: Some(json!({"event": "start"})),
            result: None,
            outcome: Outcome::Ok,
            error: None,
            decision: None,
            payload_hash: hash::payload_hash(Some(&json!({"event": "start"})), None),
            payload_truncated: false,
            payload_size_bytes: 17,
            tags: vec!["lifecycle".to_string()],
        },
        Action {
            id: action_read,
            seq: 1,
            parent_id: None,
            cause_id: Some(session_start),
            ts_wall: Utc.with_ymd_and_hms(2026, 5, 23, 14, 2, 13).unwrap()
                + chrono::Duration::milliseconds(412),
            ts_mono_ns: 2_275_000_000,
            duration_ns: Some(18_324_000),
            kind: Kind::ToolCall,
            actor: Actor::Agent,
            tool: Some("read_file".to_string()),
            args: Some(json!({"path": "/Users/x/Projects/site/package.json"})),
            result: Some(json!({
                "content": "{\"name\":\"site\"...}",
                "mime": "application/json"
            })),
            outcome: Outcome::Ok,
            error: None,
            decision: Some(SandboxDecision {
                verdict: "allow".to_string(),
                rule_id: "fs.allow.projects".to_string(),
                reason: "Path inside fs_allow".to_string(),
                mode: "enforce".to_string(),
                prompted: false,
                prompt_resolution: None,
            }),
            payload_hash: hash::payload_hash(
                Some(&json!({"path": "/Users/x/Projects/site/package.json"})),
                Some(&json!({
                    "content": "{\"name\":\"site\"...}",
                    "mime": "application/json"
                })),
            ),
            payload_truncated: false,
            payload_size_bytes: 2148,
            tags: vec!["fs".to_string(), "read".to_string()],
        },
        Action {
            id: action_denied,
            seq: 2,
            parent_id: None,
            cause_id: Some(session_start),
            ts_wall: Utc.with_ymd_and_hms(2026, 5, 23, 14, 2, 14).unwrap()
                + chrono::Duration::milliseconds(108),
            ts_mono_ns: 2_971_000_000,
            duration_ns: Some(4_102_000),
            kind: Kind::ToolCall,
            actor: Actor::Agent,
            tool: Some("write_file".to_string()),
            args: Some(json!({
                "content": "Host evil ...",
                "path": "/Users/x/.ssh/config"
            })),
            result: None,
            outcome: Outcome::Denied,
            error: Some(ActionError {
                code: "SANDBOX_DENY".to_string(),
                message: "Write to ~/.ssh/config blocked by policy".to_string(),
                source: "sandbox".to_string(),
                data: None,
            }),
            decision: Some(SandboxDecision {
                verdict: "deny".to_string(),
                rule_id: "fs.deny.ssh".to_string(),
                reason: "Path matches fs_deny pattern ~/.ssh".to_string(),
                mode: "enforce".to_string(),
                prompted: false,
                prompt_resolution: None,
            }),
            payload_hash: hash::payload_hash(
                Some(&json!({
                    "content": "Host evil ...",
                    "path": "/Users/x/.ssh/config"
                })),
                None,
            ),
            payload_truncated: false,
            payload_size_bytes: 78,
            tags: vec!["fs".to_string(), "write".to_string(), "denied".to_string()],
        },
    ];

    let mut by_outcome = std::collections::BTreeMap::new();
    by_outcome.insert("denied".to_string(), 1);
    by_outcome.insert("ok".to_string(), 2);
    let mut by_kind = std::collections::BTreeMap::new();
    by_kind.insert("session_event".to_string(), 1);
    by_kind.insert("tool_call".to_string(), 2);

    SessionFile {
        schema_version: SCHEMA_VERSION.to_string(),
        session: SessionMeta {
            id: ulid("01HXP4M9C0KTQ8N5W2Y3Z6R7AB"),
            started_at: Utc.with_ymd_and_hms(2026, 5, 23, 14, 2, 11).unwrap()
                + chrono::Duration::milliseconds(137),
            ended_at: Some(
                Utc.with_ymd_and_hms(2026, 5, 23, 14, 2, 14).unwrap()
                    + chrono::Duration::milliseconds(892),
            ),
            started_mono_ns: 0,
            app: AppInfo {
                name: "mcp-hub".to_string(),
                version: "0.1.0".to_string(),
                build: "a1b2c3d".to_string(),
                os: "macos-14.4-arm64".to_string(),
            },
            server: ServerInfo {
                id: "filesystem".to_string(),
                name: "Filesystem".to_string(),
                version: "0.6.2".to_string(),
                transport: "stdio".to_string(),
                command: vec![
                    "npx".to_string(),
                    "@modelcontextprotocol/server-filesystem".to_string(),
                    "/Users/x/Projects".to_string(),
                ],
                capabilities: vec!["tools".to_string(), "resources".to_string()],
            },
            client: ClientInfo {
                name: "claude-desktop".to_string(),
                version: "0.9.4".to_string(),
            },
            sandbox: SandboxConfig {
                mode: "enforce".to_string(),
                fs_allow: vec!["/Users/x/Projects".to_string()],
                fs_deny: vec!["~/.ssh".to_string()],
                net_allow: vec![],
                net_default: "deny".to_string(),
            },
            redactions: None,
        },
        actions,
        stats: Some(Stats {
            total_actions: 3,
            by_outcome,
            by_kind,
            denied_count: 1,
            error_count: 0,
            duration_ms: 2975,
            avg_action_ms: 11.213,
            p95_action_ms: 18.324,
            bytes_in: 0,
            bytes_out: 0,
        }),
    }
}

// ---------------------------------------------------------------------------
// Snapshot helpers
// ---------------------------------------------------------------------------

/// Build the canonical pretty-printed `session.json` bytes from typed values.
fn build_canonical_json() -> String {
    serde_json::to_string_pretty(&canonical_session()).expect("serialize canonical json")
}

/// Build the canonical NDJSON bytes (meta → action* → end, one record per line).
fn build_canonical_ndjson() -> String {
    let session_file = canonical_session();
    let session_meta = session_file.session.clone();
    let actions = session_file.actions.clone();
    let stats = session_file.stats.clone().unwrap();
    let ended_at = session_meta.ended_at.unwrap();

    let mut lines: Vec<String> = Vec::with_capacity(actions.len() + 2);
    lines.push(
        serde_json::to_string(&NdjsonRecord::Meta {
            schema_version: SCHEMA_VERSION.to_string(),
            session: session_meta,
        })
        .unwrap(),
    );
    for a in &actions {
        lines.push(serde_json::to_string(&NdjsonRecord::Action { action: a.clone() }).unwrap());
    }
    lines.push(serde_json::to_string(&NdjsonRecord::End { ended_at, stats }).unwrap());
    format!("{}\n", lines.join("\n"))
}

/// Seed both fixture files exactly once across the entire test-binary run.
///
/// `OnceLock` guarantees serial execution despite cargo's parallel test runner,
/// so the "first test wins the race and creates the file" problem can't happen.
/// With `UPDATE_FIXTURES=1` the seeder always rewrites; otherwise it only
/// creates missing files, leaving committed snapshots untouched.
static FIXTURES_SEEDED: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn ensure_fixtures() {
    FIXTURES_SEEDED.get_or_init(|| {
        let dir = fixture_dir();
        std::fs::create_dir_all(&dir).expect("create fixture dir");

        let update = std::env::var("UPDATE_FIXTURES").is_ok();

        let json_path = dir.join("canonical-session.json");
        if update || !json_path.exists() {
            std::fs::write(&json_path, build_canonical_json()).expect("write json fixture");
        }

        let ndjson_path = dir.join("canonical-session.ndjson");
        if update || !ndjson_path.exists() {
            std::fs::write(&ndjson_path, build_canonical_ndjson()).expect("write ndjson fixture");
        }
    });
}

fn read_fixture(name: &str) -> String {
    ensure_fixtures();
    let path = fixture_dir().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "fixture {} missing or unreadable ({e}). \
             Run `UPDATE_FIXTURES=1 cargo test --test session_roundtrip` to (re)create it.",
            path.display()
        )
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn json_fixture_is_byte_equal_to_serializer() {
    let expected = build_canonical_json();
    let on_disk = read_fixture("canonical-session.json");
    assert_eq!(
        on_disk.trim_end(),
        expected.trim_end(),
        "fixture out of sync with serializer — set UPDATE_FIXTURES=1 if intentional"
    );
}

#[test]
fn json_fixture_parse_reserialize_is_byte_stable() {
    // The defining roundtrip test from §13.
    let on_disk = read_fixture("canonical-session.json");
    let parsed: SessionFile = serde_json::from_str(&on_disk).expect("parse");
    let reserialized = serde_json::to_string_pretty(&parsed).expect("reserialize");

    assert_eq!(
        on_disk.trim_end(),
        reserialized.trim_end(),
        "parse → reserialize must be byte-stable"
    );
}

#[test]
fn ndjson_fixture_matches_manual_record_serialization() {
    // The NDJSON fixture is the on-disk form the SessionWriter produces.
    // We build it manually here (one compact line per record, meta → action* → end)
    // and verify it matches both (a) the on-disk fixture and (b) what the actual
    // writer emits in the next test.
    let expected = build_canonical_ndjson();
    let on_disk = read_fixture("canonical-session.ndjson");
    assert_eq!(
        on_disk, expected,
        "ndjson fixture out of sync — set UPDATE_FIXTURES=1 if intentional"
    );
}

#[test]
fn writer_produces_byte_equal_ndjson() {
    // The fixture is the contract. The real SessionWriter must produce the
    // exact same bytes given the canonical typed inputs.
    let dir = tempdir();
    let path = dir.join("session.ndjson");

    let session_file = canonical_session();
    let session_meta = session_file.session.clone();
    let actions = session_file.actions.clone();
    let stats = session_file.stats.clone().unwrap();
    let ended_at = session_meta.ended_at.unwrap();

    let mut writer =
        SessionWriter::create(&path, session_meta).expect("create writer");
    for a in &actions {
        writer.append(a).expect("append action");
    }
    writer.finalize(ended_at, stats).expect("finalize");

    let produced = std::fs::read_to_string(&path).expect("read produced file");
    let fixture = read_fixture("canonical-session.ndjson");
    assert_eq!(
        produced, fixture,
        "SessionWriter output diverged from canonical NDJSON fixture"
    );
}

#[test]
fn reader_handles_truncated_session_missing_end() {
    // Simulate the most common crash mode: process died after some actions
    // were written but before finalize().
    let session_file = canonical_session();
    let session_meta = session_file.session.clone();
    let first_two_actions = session_file.actions[..2].to_vec();

    let mut lines: Vec<String> = vec![
        serde_json::to_string(&NdjsonRecord::Meta {
            schema_version: SCHEMA_VERSION.to_string(),
            session: session_meta,
        })
        .unwrap(),
    ];
    for a in &first_two_actions {
        lines.push(
            serde_json::to_string(&NdjsonRecord::Action { action: a.clone() }).unwrap(),
        );
    }
    let bytes = format!("{}\n", lines.join("\n"));

    let outcome =
        read_ndjson_from(std::io::Cursor::new(bytes.as_bytes())).expect("reader");
    assert!(outcome.is_truncated(), "missing end record → Truncated");

    let file = outcome.into_file();
    assert_eq!(file.actions.len(), 2);
    assert!(file.session.ended_at.is_none(), "truncated → ended_at None");
    assert_eq!(file.stats.as_ref().unwrap().total_actions, 2);
}

#[test]
fn reader_tolerates_partial_last_line() {
    // The other crash mode: fsync was mid-flight, last record is half-written.
    let session_file = canonical_session();
    let session_meta = session_file.session.clone();

    let meta_line = serde_json::to_string(&NdjsonRecord::Meta {
        schema_version: SCHEMA_VERSION.to_string(),
        session: session_meta,
    })
    .unwrap();
    let action_line = serde_json::to_string(&NdjsonRecord::Action {
        action: session_file.actions[0].clone(),
    })
    .unwrap();
    // Truncate the action line halfway through to simulate torn write.
    let half = &action_line[..action_line.len() / 2];

    let bytes = format!("{}\n{}\n{}", meta_line, action_line, half);

    let outcome =
        read_ndjson_from(std::io::Cursor::new(bytes.as_bytes())).expect("reader");
    assert!(outcome.is_truncated());
    let file = outcome.into_file();
    // We get the one complete action, partial line is silently dropped.
    assert_eq!(file.actions.len(), 1);
}

#[test]
fn reader_sorts_actions_by_seq_regardless_of_disk_order() {
    // Once the writer becomes an mpsc task (step 2), completions can arrive
    // out of order under concurrent in-flight requests, so the on-disk NDJSON
    // line order is no longer the semantic order. Reader must surface a
    // seq-sorted view per spec §3.
    let session_file = canonical_session();
    let session_meta = session_file.session.clone();

    // Reverse-order the action lines on purpose.
    let mut lines: Vec<String> = vec![serde_json::to_string(&NdjsonRecord::Meta {
        schema_version: SCHEMA_VERSION.to_string(),
        session: session_meta,
    })
    .unwrap()];
    for a in session_file.actions.iter().rev() {
        lines.push(
            serde_json::to_string(&NdjsonRecord::Action { action: a.clone() }).unwrap(),
        );
    }
    let bytes = format!("{}\n", lines.join("\n"));

    let outcome =
        read_ndjson_from(std::io::Cursor::new(bytes.as_bytes())).expect("reader");
    let file = outcome.into_file();

    let seqs: Vec<u64> = file.actions.iter().map(|a| a.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2], "reader must sort by seq ascending");
}

#[test]
fn writer_per_record_fsync_still_parseable() {
    // Stress: force fsync after every record (worst case for performance,
    // best case for crash safety). Output must still be a valid session.
    let dir = tempdir();
    let path = dir.join("session.ndjson");

    let session_file = canonical_session();
    let mut writer = SessionWriter::create(&path, session_file.session.clone())
        .expect("create")
        .with_fsync_batch(1);
    for a in &session_file.actions {
        writer.append(a).expect("append");
    }
    writer
        .finalize(
            session_file.session.ended_at.unwrap(),
            session_file.stats.clone().unwrap(),
        )
        .expect("finalize");

    let outcome = read_ndjson(&path).expect("read");
    assert!(matches!(outcome, ReadOutcome::Complete(_)));
    let file = outcome.into_file();
    assert_eq!(file.actions.len(), 3);
}

#[test]
fn writer_drop_without_finalize_leaves_truncated_but_parseable_file() {
    // The implicit guarantee: even if the writer is dropped (or the process
    // panics) without finalize(), what's already on disk plus fsynced batches
    // is recoverable by the reader.
    let dir = tempdir();
    let path = dir.join("session.ndjson");

    let session_file = canonical_session();
    {
        let mut writer = SessionWriter::create(&path, session_file.session.clone())
            .expect("create")
            .with_fsync_batch(1);
        writer.append(&session_file.actions[0]).expect("append 1");
        writer.append(&session_file.actions[1]).expect("append 2");
        // No finalize — writer dropped here.
    }

    let outcome = read_ndjson(&path).expect("read");
    assert!(outcome.is_truncated(), "no end record → Truncated");
    let file = outcome.into_file();
    assert_eq!(file.actions.len(), 2);
}

// ---------------------------------------------------------------------------
// Tiny tempdir helper — we don't want to pull in the `tempfile` crate just
// for tests. CARGO_TARGET_TMPDIR is set by cargo and cleaned between runs.
// ---------------------------------------------------------------------------

fn tempdir() -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let unique = format!(
        "mcp-hub-session-test-{}-{}",
        std::process::id(),
        Ulid::new()
    );
    let dir = base.join(unique);
    std::fs::create_dir_all(&dir).expect("create tempdir");
    dir
}
