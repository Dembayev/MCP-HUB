//! Concurrency stress tests for the session writer pipeline.
//!
//! These tests are the load-bearing contract for the runtime layer added in
//! step 2: under N concurrent producers with randomized delay, EVERY `seq` in
//! the range `0..N` must appear in the resulting NDJSON file exactly once
//! (no loss, no duplicates), and the reader must return them in `seq`
//! ascending order regardless of which physical order they hit the disk.
//!
//! The on-disk lines may be in any arrival order — that's the whole point of
//! the mpsc decoupling. Semantic ordering is owned by `seq` + the reader's
//! sort. This file tests both invariants together.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use chrono::{TimeZone, Utc};
use serde_json::json;
use tokio::time::sleep;
use ulid::Ulid;

use mcp_hub_lib::session::*;

// ---------------------------------------------------------------------------
// Helpers — duplicated locally rather than shared with session_roundtrip.rs
// because cargo treats each `tests/*.rs` file as its own binary.
// ---------------------------------------------------------------------------

fn tempdir() -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let unique = format!(
        "mcp-hub-session-concurrency-{}-{}",
        std::process::id(),
        Ulid::new()
    );
    let dir = base.join(unique);
    std::fs::create_dir_all(&dir).expect("create tempdir");
    dir
}

fn minimal_meta() -> SessionMeta {
    SessionMeta {
        id: Ulid::new(),
        started_at: Utc.with_ymd_and_hms(2026, 5, 23, 14, 0, 0).unwrap(),
        ended_at: None,
        started_mono_ns: 0,
        app: AppInfo {
            name: "mcp-hub".into(),
            version: "0.1.0".into(),
            build: "test".into(),
            os: "test-os".into(),
        },
        server: ServerInfo {
            id: "stress".into(),
            name: "Stress".into(),
            version: "0.0.0".into(),
            transport: "stdio".into(),
            command: vec!["echo".into()],
            capabilities: vec![],
        },
        client: ClientInfo {
            name: "unknown".into(),
            version: "0".into(),
        },
        sandbox: SandboxConfig {
            mode: "observe".into(),
            fs_allow: vec![],
            fs_deny: vec![],
            net_allow: vec![],
            net_default: "allow".into(),
        },
        redactions: None,
    }
}

/// Deterministic 0..bound pseudo-random from a seed. We deliberately avoid
/// `rand` as a dependency — the LCG is sufficient for "spread completions out
/// in time" and keeps the test reproducible across runs and machines.
fn lcg_jitter_micros(seq: u64, bound: u64) -> u64 {
    seq.wrapping_mul(2_654_435_761) % bound
}

/// Build a Partial → Action for the test stream. The args/result are minimal;
/// the point is to exercise the pipeline, not realistic payload shape.
fn make_action(seq: u64) -> Action {
    let partial = PartialAction::tool_call(
        seq,
        "stress.op".into(),
        Some(json!({"seq": seq})),
        Utc.with_ymd_and_hms(2026, 5, 23, 14, 0, 0).unwrap()
            + chrono::Duration::microseconds(seq as i64),
        seq * 1_000, // ts_mono_ns: monotonically increasing with seq
        None,
    );
    partial.complete(Some(json!({"ok": true})), 1_000, None)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The headline stress test from §13 step 2.
///
/// Spawn N concurrent tokio tasks, each holding its own cloned
/// `SessionAppender`. Each task sleeps for a deterministic-but-spread jitter
/// (0–5ms) before sending one Action. After joining all tasks, finalize the
/// session and read it back through the canonical reader. Assert:
///
/// 1. The number of actions read equals N (no loss).
/// 2. The `seq` values form a contiguous `0..N` set (no gaps, no duplicates).
/// 3. The reader returns them in `seq` ascending order (decoupling holds).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn writer_task_no_loss_under_concurrent_appends() {
    const N: u64 = 1_000;

    let dir = tempdir();
    let path = dir.join("stress.ndjson");
    let handle = SessionHandle::spawn(path.clone(), minimal_meta())
        .await
        .expect("spawn writer");
    let appender = handle.appender();

    let mut joins = Vec::with_capacity(N as usize);
    for seq in 0..N {
        let appender = appender.clone();
        joins.push(tokio::spawn(async move {
            // 0–5ms deterministic jitter, ensures completions race.
            let jitter = lcg_jitter_micros(seq, 5_000);
            sleep(Duration::from_micros(jitter)).await;
            appender
                .append(make_action(seq))
                .await
                .expect("append in stress task");
        }));
    }
    // Drop the supervising appender clone too, so the only remaining tx is
    // the one inside the handle (which will be dropped by finalize).
    drop(appender);
    for j in joins {
        j.await.expect("stress task panicked");
    }

    let path = handle
        .finalize(Utc::now(), Stats::default())
        .await
        .expect("finalize");

    let outcome = read_ndjson(&path).expect("read");
    assert!(
        matches!(outcome, ReadOutcome::Complete(_)),
        "session should be Complete after explicit finalize"
    );
    let file = outcome.into_file();

    assert_eq!(
        file.actions.len() as u64,
        N,
        "lost actions under concurrent append"
    );

    let seqs: Vec<u64> = file.actions.iter().map(|a| a.seq).collect();
    for (i, s) in seqs.iter().enumerate() {
        assert_eq!(
            *s, i as u64,
            "seq mismatch at position {i}: got {s}, expected {i} \
             (gap, duplicate, or unsorted)"
        );
    }
    let unique: HashSet<u64> = seqs.iter().copied().collect();
    assert_eq!(unique.len() as u64, N, "duplicate seq detected");
}

/// Same harness but with a very small channel capacity (4). Forces real
/// backpressure: producers `await` on `tx.send` until the writer drains
/// space. Verifies backpressure doesn't drop messages or deadlock.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn writer_task_backpressure_does_not_lose_messages() {
    const N: u64 = 500;

    let dir = tempdir();
    let path = dir.join("backpressure.ndjson");
    let handle = SessionHandle::spawn_with_capacity(path.clone(), minimal_meta(), 4)
        .await
        .expect("spawn writer with tiny channel");
    let appender = handle.appender();

    let mut joins = Vec::with_capacity(N as usize);
    for seq in 0..N {
        let appender = appender.clone();
        joins.push(tokio::spawn(async move {
            appender
                .append(make_action(seq))
                .await
                .expect("append under backpressure");
        }));
    }
    drop(appender);
    for j in joins {
        j.await.unwrap();
    }

    let path = handle
        .finalize(Utc::now(), Stats::default())
        .await
        .expect("finalize");

    let file = read_ndjson(&path).expect("read").into_file();
    assert_eq!(file.actions.len() as u64, N);
    for (i, a) in file.actions.iter().enumerate() {
        assert_eq!(a.seq, i as u64, "seq {} out of order at {}", a.seq, i);
    }
}

/// If producers are dropped without finalize being called, the session file
/// must still be a valid (truncated) session — the reader returns
/// `ReadOutcome::Truncated` and stats are recomputed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_handle_without_finalize_yields_truncated_session() {
    const N: u64 = 50;

    let dir = tempdir();
    let path = dir.join("drop-no-finalize.ndjson");

    {
        let handle = SessionHandle::spawn(path.clone(), minimal_meta())
            .await
            .expect("spawn");
        let appender = handle.appender();
        for seq in 0..N {
            appender.append(make_action(seq)).await.expect("append");
        }
        // Drop appender + handle without finalize.
        drop(appender);
        drop(handle);
    }

    // Give the writer task a moment to drain pending messages and exit its
    // recv loop. In a real proxy this happens naturally as part of join.
    sleep(Duration::from_millis(50)).await;

    let outcome = read_ndjson(&path).expect("read");
    assert!(
        outcome.is_truncated(),
        "session without finalize must be Truncated"
    );
    let file = outcome.into_file();
    assert_eq!(file.actions.len() as u64, N);
    assert!(file.session.ended_at.is_none());
}

/// Sending Append AFTER Finalize must NOT corrupt the file or deadlock.
/// (It does silently lose the message, which is documented in the
/// SessionHandle::finalize contract — but the file itself stays clean.)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn append_after_finalize_is_dropped_not_corrupting() {
    let dir = tempdir();
    let path = dir.join("post-finalize.ndjson");

    let handle = SessionHandle::spawn(path.clone(), minimal_meta())
        .await
        .expect("spawn");
    let appender = handle.appender();

    appender.append(make_action(0)).await.expect("first append");

    let final_path = handle
        .finalize(Utc::now(), Stats::default())
        .await
        .expect("finalize");

    // Writer task has returned; channel is closed. Subsequent appends fail
    // gracefully with the writer-task-gone error rather than corrupting.
    let post = appender.append(make_action(1)).await;
    assert!(post.is_err(), "append after finalize must surface error");

    let file = read_ndjson(&final_path).expect("read").into_file();
    // Exactly the one pre-finalize action survives. No phantom action 1.
    assert_eq!(file.actions.len(), 1);
    assert_eq!(file.actions[0].seq, 0);
}
