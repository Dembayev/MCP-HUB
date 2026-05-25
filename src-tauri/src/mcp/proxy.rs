//! MCP proxy runtime.
//!
//! Architecture:
//!
//! ```text
//!   AI client (Claude Desktop)
//!         │  stdio
//!         ▼
//!   mcp-hub-proxy (tiny standalone binary)
//!         │  Unix domain socket
//!         ▼
//!   MCP Hub (this module)
//!         │  stdio
//!         ▼
//!   real MCP server (npx/python/…)
//! ```
//!
//! For each connection we spawn a dedicated child via
//! [`McpManager::spawn_for_proxy`] and bidirectionally splice bytes
//! between the socket and the child's stdio. Every JSON-RPC frame
//! flowing through is parsed:
//!
//! - `tools/call` request → emit `Pending` AgentAction with stable id,
//!   classify the action, check permissions against the granted set, then
//!   either deny (synthesize a JSON-RPC error response back to the client
//!   and skip forwarding) or forward verbatim.
//! - matching response → update the same AgentAction to `Success` or
//!   `Failed` with the round-trip latency.
//!
//! Reads from a Unix-domain socket only. Windows support will land via
//! named pipes; [`run_listener`] is a no-op there for now so the Tauri
//! app still builds.

#![cfg_attr(not(unix), allow(unused_imports, dead_code))]

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Utc};
use parking_lot::{Mutex, RwLock};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};
use ulid::Ulid;

use crate::commands::approvals::EVENT_APPROVAL_REQUESTED;
use crate::db::permissions::{self, PersistedPermission, RequestedPermission};
use crate::error::{AppError, AppResult};
use crate::mcp::agent::{
    classify_jsonrpc, classify_sandbox_denial, ActionKind, ActionStatus, AgentAction,
};
use crate::mcp::logs::{LogEntry, LogStream};
use crate::security::{risk_for_kind, ApprovalDecision, ApprovalRequest as ApprovalReqPayload};
use crate::session::{
    ActionError as SessionActionError, AppInfo as SessionAppInfo, ClientInfo as SessionClientInfo,
    PartialAction, SandboxConfig as SessionSandboxConfig, SandboxDecision as SessionSandboxDecision,
    ServerInfo as SessionServerInfo, SessionAppender, SessionHandle, SessionMeta, Stats as SessionStats,
};
use crate::state::AppState;

pub const EVENT_AGENT_ACTION: &str = "agent-action";
pub const EVENT_SERVER_LOG: &str = "server-log";

#[cfg(unix)]
pub async fn run_listener(
    sock_path: PathBuf,
    state: Arc<AppState>,
    app: AppHandle,
) -> AppResult<()> {
    use tokio::net::UnixListener;

    // Remove any stale socket from a previous run.
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path).map_err(AppError::from)?;
    tracing::info!(?sock_path, "MCP proxy listening");

    loop {
        let (stream, _addr) = listener.accept().await.map_err(AppError::from)?;
        let state = state.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = handle_connection(stream, state, app).await {
                tracing::warn!(error = %err, "proxy connection ended with error");
            }
        });
    }
}

#[cfg(not(unix))]
pub async fn run_listener(
    _sock_path: PathBuf,
    _state: Arc<AppState>,
    _app: AppHandle,
) -> AppResult<()> {
    tracing::warn!("proxy listener is Unix-only for now; skipping");
    Ok(())
}

#[cfg(unix)]
async fn handle_connection(
    stream: tokio::net::UnixStream,
    state: Arc<AppState>,
    app: AppHandle,
) -> AppResult<()> {
    let (sock_read, sock_write) = stream.into_split();

    // -----------------------------------------------------------------
    // Centralized socket writer. Both directions of the splice (server→
    // client forwarding AND denial responses synthesised from the
    // client→server pump) push pre-framed lines into this channel; a
    // single task owns `sock_write` and drains the queue. This sidesteps
    // the "two writers to one TCP/UDS half" problem cleanly.
    // -----------------------------------------------------------------
    let (write_tx, mut write_rx) = mpsc::channel::<Vec<u8>>(64);
    {
        let mut sock_write = sock_write;
        tauri::async_runtime::spawn(async move {
            while let Some(bytes) = write_rx.recv().await {
                if sock_write.write_all(&bytes).await.is_err() {
                    break;
                }
                if sock_write.flush().await.is_err() {
                    break;
                }
            }
        });
    }

    let mut sock_lines = BufReader::new(sock_read).lines();

    // Greeting: client sends "<server-id>\n" before any JSON-RPC.
    let greeting = match sock_lines.next_line().await {
        Ok(Some(line)) => line,
        _ => return Ok(()),
    };
    let server_id = greeting.trim().to_string();
    if server_id.is_empty() {
        return Ok(());
    }

    let server = state.mcp.registry().get(&server_id)?;
    let mut child = state.mcp.spawn_for_proxy(&server_id)?;
    let mut child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Internal("child has no stdin".into()))?;
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Internal("child has no stdout".into()))?;
    let child_stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Internal("child has no stderr".into()))?;

    tracing::info!(server = %server.name, "proxy connection established");
    let _ = app.emit(
        "server-status-changed",
        json!({"id": &server_id, "status": "running"}),
    );

    let granted: Arc<RwLock<Vec<PersistedPermission>>> = Arc::new(RwLock::new(
        permissions::list_for_server(state.mcp.db(), &server_id).unwrap_or_default(),
    ));
    let server_name = server.name.clone();
    let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));

    // ----------------------------------------------------------------------
    // Session trace setup (step 6 — real proxy instrumentation).
    //
    // Each proxy connection produces one NDJSON session file under
    // <data_dir>/sessions/<ulid>.ndjson. The Timeline UI tails this and
    // exposes scrubber + replay over the entire run.
    //
    // The first Action emitted is a `session_event: start` lifecycle
    // record; its ULID is reused as the `cause_id` for every downstream
    // action so the trace's causal chain is well-formed from the start.
    // ----------------------------------------------------------------------
    let session_anchor = Instant::now();
    let session_seq = Arc::new(AtomicU64::new(0));
    let session_id = Ulid::new();
    let session_path = state.sessions_dir.join(format!("{session_id}.ndjson"));
    let session_started_at = Utc::now();
    let session_meta = {
        // Hold the read guard only for the duration of this snapshot — release
        // before any await so we never bridge a sync RwLock across a yield.
        let g = granted.read();
        build_session_meta(session_id, session_started_at, &server, &g)
    };

    let session_handle = match SessionHandle::spawn(session_path.clone(), session_meta).await {
        Ok(h) => Some(h),
        Err(err) => {
            // Failing to open the session file is not fatal — the proxy
            // still functions; the Timeline tab just won't see this run.
            tracing::warn!(error = %err, ?session_path, "session writer spawn failed");
            None
        }
    };
    let session_appender = session_handle.as_ref().map(|h| h.appender());

    // Emit the lifecycle "start" record. Its id becomes the cause_id for
    // every classified action in this session.
    let cause_id = if let Some(appender) = &session_appender {
        let seq = session_seq.fetch_add(1, Ordering::SeqCst);
        let partial = PartialAction::lifecycle(seq, "start", session_started_at, 0);
        let id = partial.id;
        let action = partial.complete(None, 0, None);
        if let Err(e) = appender.append(action).await {
            tracing::warn!(error = %e, "session append start failed");
        }
        Some(id)
    } else {
        None
    };

    // Lightweight counters for the finalize stats. Updated from both pumps
    // via `parking_lot::Mutex` — these are tiny scalar increments, not a
    // hot-path lock.
    let session_counters = Arc::new(Mutex::new(SessionCounters::default()));

    // --- stderr drain task -------------------------------------------------
    {
        let app = app.clone();
        let actions = state.mcp.actions_arc();
        let logs = state.mcp.logs_arc();
        let server_id = server_id.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(child_stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let entry = LogEntry::new(server_id.clone(), LogStream::Stderr, line.clone());
                logs.push(&entry);
                let _ = app.emit(EVENT_SERVER_LOG, &entry);
                if let Some(action) = classify_sandbox_denial(&line, &server_id) {
                    actions.push_or_update(&action);
                    let _ = app.emit(EVENT_AGENT_ACTION, &action);
                }
            }
        });
    }

    // --- client → server ----------------------------------------------------
    let actions_for_req = state.mcp.actions_arc();
    let logs_for_req = state.mcp.logs_arc();
    let pending_for_req = pending.clone();
    let app_for_req = app.clone();
    let server_id_for_req = server_id.clone();
    let server_name_for_req = server_name.clone();
    let granted_for_req = granted.clone();
    let state_for_req = state.clone();
    let write_tx_for_req = write_tx.clone();
    let session_appender_for_req = session_appender.clone();
    let session_seq_for_req = session_seq.clone();
    let session_counters_for_req = session_counters.clone();

    let client_to_server = async move {
        while let Some(line) = sock_lines.next_line().await? {
            // Mirror to LogSink so the Activity view shows exactly what the
            // client sent, even if we end up denying the forward.
            let entry = LogEntry::new(server_id_for_req.clone(), LogStream::Stdout, line.clone());
            logs_for_req.push(&entry);

            match classify_jsonrpc(&line, &server_id_for_req) {
                Some(mut action) => {
                    let rpc_id = action.request_id.clone();
                    let request_started = Instant::now();

                    // Build the session-trace counterpart up front. ts_mono_ns
                    // is captured at request-arrival time; duration_ns is
                    // filled in on completion (response or denial).
                    let partial_seq = session_seq_for_req.fetch_add(1, Ordering::SeqCst);
                    let partial = PartialAction::tool_call(
                        partial_seq,
                        action.tool_name.clone(),
                        action.params.clone(),
                        Utc::now(),
                        elapsed_ns(session_anchor),
                        cause_id,
                    )
                    .with_tag(action_kind_tag(action.kind));

                    // Read the granted snapshot under a brief lock; drop the
                    // guard before any await so we never hold a sync RwLock
                    // across yield points.
                    let decision = {
                        let g = granted_for_req.read();
                        enforce_permission(action.kind, action.target.as_deref(), &g)
                    };

                    let resolved: Result<(), DenyResult> = match decision {
                        Decision::Allow => Ok(()),
                        Decision::AskUser { scope } => {
                            // Surface a Pending AgentAction so the UI lights
                            // up immediately — the approval modal is the gate,
                            // but the live activity view should reflect that
                            // a request is in flight.
                            action.status = ActionStatus::Pending;
                            actions_for_req.push_or_update(&action);
                            let _ = app_for_req.emit(EVENT_AGENT_ACTION, &action);

                            await_user_approval(
                                &state_for_req,
                                &app_for_req,
                                &granted_for_req,
                                &server_id_for_req,
                                &server_name_for_req,
                                &action,
                                scope,
                            )
                            .await
                        }
                    };

                    match resolved {
                        Ok(()) => {
                            action.status = ActionStatus::Pending;
                            actions_for_req.push_or_update(&action);
                            let _ = app_for_req.emit(EVENT_AGENT_ACTION, &action);

                            if let Some(id_val) = rpc_id {
                                pending_for_req.lock().insert(
                                    rpc_id_key(&id_val),
                                    InFlight {
                                        action: action.clone(),
                                        partial,
                                        started: request_started,
                                    },
                                );
                            } else {
                                // No rpc id → no response will come back. Emit
                                // the trace action immediately as a successful
                                // fire-and-forget.
                                if let Some(appender) = &session_appender_for_req {
                                    let trace_action = partial.complete(None, 0, None);
                                    session_counters_for_req
                                        .lock()
                                        .record(trace_action.outcome, trace_action.duration_ns);
                                    let _ = appender.append(trace_action).await;
                                }
                            }

                            // Forward verbatim.
                            let mut payload = line.into_bytes();
                            payload.push(b'\n');
                            child_stdin.write_all(&payload).await?;
                            child_stdin.flush().await?;
                        }
                        Err(DenyResult { scope, reason }) => {
                            action.status = ActionStatus::Denied;
                            action.denied_reason =
                                Some(format!("{} (required scope: {})", reason, scope));
                            actions_for_req.push_or_update(&action);
                            let _ = app_for_req.emit(EVENT_AGENT_ACTION, &action);

                            // Emit denial into the session trace. Prompted=true
                            // because the user actively saw the modal and chose
                            // Deny (or the channel was dropped — still attributed
                            // to the user-mediated path).
                            if let Some(appender) = &session_appender_for_req {
                                let decision_rec = SessionSandboxDecision {
                                    verdict: "deny".into(),
                                    rule_id: scope.to_string(),
                                    reason: reason.clone(),
                                    mode: "enforce".into(),
                                    prompted: true,
                                    prompt_resolution: Some("deny".into()),
                                };
                                let err_rec = SessionActionError {
                                    code: "SANDBOX_DENY".into(),
                                    message: reason.clone(),
                                    source: "sandbox".into(),
                                    data: None,
                                };
                                let duration_ns = request_started.elapsed().as_nanos() as u64;
                                let trace_action = partial.complete_denied(
                                    decision_rec,
                                    err_rec,
                                    Some(duration_ns),
                                );
                                session_counters_for_req
                                    .lock()
                                    .record(trace_action.outcome, trace_action.duration_ns);
                                let _ = appender.append(trace_action).await;
                            }

                            if let Some(rpc_id) = rpc_id {
                                let err_resp = json!({
                                    "jsonrpc": "2.0",
                                    "id": rpc_id,
                                    "error": {
                                        "code": -32603,
                                        "message": "Permission denied by MCP Hub",
                                        "data": { "scope": scope, "reason": reason }
                                    }
                                });
                                let mut payload = err_resp.to_string().into_bytes();
                                payload.push(b'\n');
                                let _ = write_tx_for_req.send(payload).await;
                            }
                        }
                    }
                }
                None => {
                    // Not a classifiable JSON-RPC line — forward verbatim.
                    let mut payload = line.into_bytes();
                    payload.push(b'\n');
                    child_stdin.write_all(&payload).await?;
                    child_stdin.flush().await?;
                }
            }
        }
        Ok::<(), AppError>(())
    };

    // --- server → client ----------------------------------------------------
    let actions_for_resp = state.mcp.actions_arc();
    let logs_for_resp = state.mcp.logs_arc();
    let pending_for_resp = pending.clone();
    let app_for_resp = app.clone();
    let server_id_for_resp = server_id.clone();
    let write_tx_for_resp = write_tx.clone();
    let session_appender_for_resp = session_appender.clone();
    let session_counters_for_resp = session_counters.clone();
    let mut child_stdout_lines = BufReader::new(child_stdout).lines();

    let server_to_client = async move {
        while let Some(line) = child_stdout_lines.next_line().await? {
            let entry =
                LogEntry::new(server_id_for_resp.clone(), LogStream::Stdout, line.clone());
            logs_for_resp.push(&entry);

            if let Some(matched) = match_response(&line, &pending_for_resp) {
                actions_for_resp.push_or_update(&matched.action);
                let _ = app_for_resp.emit(EVENT_AGENT_ACTION, &matched.action);

                if let Some(appender) = &session_appender_for_resp {
                    session_counters_for_resp
                        .lock()
                        .record(matched.trace.outcome, matched.trace.duration_ns);
                    let _ = appender.append(matched.trace).await;
                }
            }

            let mut payload = line.into_bytes();
            payload.push(b'\n');
            if write_tx_for_resp.send(payload).await.is_err() {
                break;
            }
        }
        Ok::<(), AppError>(())
    };

    let (a, b) = tokio::join!(client_to_server, server_to_client);
    if let Err(e) = a {
        tracing::debug!(error = %e, "client→server pump ended");
    }
    if let Err(e) = b {
        tracing::debug!(error = %e, "server→client pump ended");
    }

    // Best-effort: terminate the child and notify the UI.
    let _ = child.kill().await;
    let _ = app.emit(
        "server-status-changed",
        json!({"id": &server_id, "status": "stopped"}),
    );

    // Finalize the session trace. Both pumps have completed; their cloned
    // appenders have been dropped already, so the only remaining sender is
    // the one inside the SessionHandle. finalize() consumes it, writes the
    // end record + fsync, and the writer task exits cleanly.
    if let Some(handle) = session_handle {
        let stats = session_counters.lock().to_stats(session_anchor);
        if let Err(e) = handle.finalize(Utc::now(), stats).await {
            tracing::warn!(error = %e, "session finalize failed");
        }
    }

    tracing::info!(server = %server.name, "proxy connection closed");
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type PendingMap = Arc<Mutex<HashMap<String, InFlight>>>;

struct InFlight {
    action: AgentAction,
    /// Session-trace counterpart of `action`. Folded on response (success /
    /// failure) or denial via `complete*` methods, then sent to the writer.
    partial: PartialAction,
    started: Instant,
}

/// Lightweight running tally used to compute Stats at finalize() time.
/// Avoids re-parsing the NDJSON file just to count outcomes.
#[derive(Default)]
struct SessionCounters {
    total: u64,
    ok: u64,
    denied: u64,
    error: u64,
    durations_ms: Vec<f64>,
}

impl SessionCounters {
    fn record(&mut self, outcome: crate::session::Outcome, duration_ns: Option<u64>) {
        use crate::session::Outcome;
        self.total += 1;
        match outcome {
            Outcome::Ok => self.ok += 1,
            Outcome::Denied => self.denied += 1,
            Outcome::Error => self.error += 1,
            _ => {}
        }
        if let Some(ns) = duration_ns {
            self.durations_ms.push(ns as f64 / 1_000_000.0);
        }
    }

    fn to_stats(&self, anchor: Instant) -> SessionStats {
        let duration_ms = anchor.elapsed().as_millis() as u64;

        let mut sorted = self.durations_ms.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let avg_action_ms = if sorted.is_empty() {
            0.0
        } else {
            sorted.iter().sum::<f64>() / sorted.len() as f64
        };
        let p95_action_ms = if sorted.is_empty() {
            0.0
        } else {
            let rank = ((sorted.len() - 1) as f64 * 0.95).round() as usize;
            sorted[rank]
        };

        let mut by_outcome: BTreeMap<String, u64> = BTreeMap::new();
        if self.ok > 0 {
            by_outcome.insert("ok".to_string(), self.ok);
        }
        if self.denied > 0 {
            by_outcome.insert("denied".to_string(), self.denied);
        }
        if self.error > 0 {
            by_outcome.insert("error".to_string(), self.error);
        }

        // Lightweight by_kind — counters don't track kinds individually; we
        // approximate as 1 lifecycle event + remainder as tool_calls. Good
        // enough for the Timeline stats line; not authoritative.
        let mut by_kind: BTreeMap<String, u64> = BTreeMap::new();
        if self.total > 0 {
            by_kind.insert("session_event".to_string(), 1);
            if self.total > 1 {
                by_kind.insert("tool_call".to_string(), self.total - 1);
            }
        }

        SessionStats {
            total_actions: self.total,
            by_outcome,
            by_kind,
            denied_count: self.denied,
            error_count: self.error,
            duration_ms,
            avg_action_ms,
            p95_action_ms,
            bytes_in: 0,
            bytes_out: 0,
        }
    }
}

fn elapsed_ns(anchor: Instant) -> u64 {
    anchor.elapsed().as_nanos() as u64
}

/// ActionKind serializes as kebab-case ("fs-read", "fs-write", …) — use that
/// as the tag value so the session-trace inherits the same vocabulary as the
/// live AgentAction UI.
fn action_kind_tag(kind: ActionKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Construct the session's `meta` record from server registry + current
/// granted-permission snapshot. Snapshotted at connection start; the runtime
/// flow may grant more scopes during the session, but the trace metadata
/// reflects "what was granted when the session began".
fn build_session_meta(
    id: Ulid,
    started_at: DateTime<Utc>,
    server: &crate::db::models::McpServer,
    granted: &[PersistedPermission],
) -> SessionMeta {
    let mut command = vec![server.command.clone()];
    command.extend(server.args.iter().cloned());

    SessionMeta {
        id,
        started_at,
        ended_at: None,
        started_mono_ns: 0,
        app: SessionAppInfo {
            name: "mcp-hub".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            build: option_env!("VERGEN_GIT_SHA").unwrap_or("dev").to_string(),
            os: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        },
        server: SessionServerInfo {
            id: server.id.clone(),
            name: server.name.clone(),
            version: server.version.clone().unwrap_or_else(|| "unknown".into()),
            transport: server.transport.as_str().to_string(),
            command,
            capabilities: Vec::new(),
        },
        client: SessionClientInfo {
            // Real client identity (claude-desktop / cursor / …) needs the
            // mcp-hub-proxy worker to announce it in the greeting line; until
            // that protocol extension lands, "unknown" is honest.
            name: "unknown".to_string(),
            version: "0".to_string(),
        },
        sandbox: build_sandbox_snapshot(granted),
        redactions: None,
    }
}

fn build_sandbox_snapshot(granted: &[PersistedPermission]) -> SessionSandboxConfig {
    let mut fs_allow = Vec::new();
    let mut net_allow = Vec::new();
    for p in granted.iter().filter(|p| p.granted) {
        match p.scope.as_str() {
            "fs.read" | "fs.write" => {
                if let Some(t) = &p.target {
                    fs_allow.push(t.clone());
                }
            }
            "internet" | "net.outbound" => {
                net_allow.push(p.target.clone().unwrap_or_else(|| "*".to_string()));
            }
            _ => {}
        }
    }
    SessionSandboxConfig {
        mode: "enforce".to_string(),
        fs_allow,
        fs_deny: vec![
            "~/.ssh".to_string(),
            "~/.aws".to_string(),
            "/etc/".to_string(),
        ],
        net_allow,
        net_default: "deny".to_string(),
    }
}

fn rpc_id_key(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Result of pairing a server response with a pending request.
struct ResponseMatch {
    /// Updated AgentAction for the live UI flow.
    action: AgentAction,
    /// Folded session-trace Action — fully populated, ready to send to the
    /// SessionAppender. Result/error data and duration_ns are filled in here.
    trace: crate::session::Action,
}

fn match_response(line: &str, pending: &PendingMap) -> Option<ResponseMatch> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('{') {
        return None;
    }
    let value: Value = serde_json::from_str(trimmed).ok()?;
    // Responses carry an `id` plus `result` or `error` (and no `method`).
    if value.get("method").is_some() {
        return None;
    }
    let id_val = value.get("id")?;
    let key = rpc_id_key(id_val);

    let mut map = pending.lock();
    let in_flight = map.remove(&key)?;

    let duration_ns = in_flight.started.elapsed().as_nanos() as u64;
    let latency_ms = duration_ns / 1_000_000;
    let mut action = in_flight.action;
    action.latency_ms = Some(latency_ms);
    action.timestamp = Utc::now();

    let trace = if let Some(error) = value.get("error") {
        action.status = ActionStatus::Failed;
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("server error")
            .to_string();
        action.error = Some(message.clone());

        let code = error
            .get("code")
            .and_then(Value::as_i64)
            .map(|c| c.to_string())
            .unwrap_or_else(|| "MCP_ERROR".to_string());
        let session_err = SessionActionError {
            code,
            message,
            source: "tool".to_string(),
            data: error.get("data").cloned(),
        };
        in_flight.partial.complete_error(session_err, duration_ns)
    } else {
        action.status = ActionStatus::Success;
        let result = value.get("result").cloned();
        in_flight.partial.complete(result, duration_ns, None)
    };

    Some(ResponseMatch { action, trace })
}

// ---------------------------------------------------------------------------
// Permission gate
// ---------------------------------------------------------------------------

enum Decision {
    /// Scope already granted — proceed without asking.
    Allow,
    /// Scope required but not granted; surface the runtime approval modal
    /// to the user and route based on their decision.
    AskUser { scope: &'static str },
}

/// Post-resolution decision after any approval await. Collapses both the
/// pre-resolved `Decision::Allow` path and the user-clicked-Allow path into
/// `Ok(())`, and the user-clicked-Deny / channel-dropped path into
/// `Err(DenyResult)`. This lets the proxy run a single forward / denial
/// block regardless of which path produced the outcome.
struct DenyResult {
    scope: &'static str,
    reason: String,
}

fn enforce_permission(
    kind: ActionKind,
    _target: Option<&str>,
    granted: &[PersistedPermission],
) -> Decision {
    let required: Option<&'static str> = match kind {
        ActionKind::FsRead => Some("fs.read"),
        ActionKind::FsWrite => Some("fs.write"),
        ActionKind::HttpFetch | ActionKind::BrowserOpen | ActionKind::Search => Some("internet"),
        ActionKind::TerminalExec => Some("terminal"),
        // Memory and generic tool calls don't have a single mapped scope —
        // let the sandbox be the gate.
        ActionKind::MemoryStore | ActionKind::ToolCall | ActionKind::Other => None,
    };

    let Some(required) = required else {
        return Decision::Allow;
    };

    let any_grant = granted.iter().any(|p| {
        if !p.granted {
            return false;
        }
        if p.scope == required {
            return true;
        }
        // Accept `net.outbound` as a synonym for `internet`.
        required == "internet" && p.scope == "net.outbound"
    });

    if any_grant {
        Decision::Allow
    } else {
        // Missing scope → ask the user. Pre-step-5 this was a hard Deny.
        Decision::AskUser { scope: required }
    }
}

// ---------------------------------------------------------------------------
// Runtime approval flow
// ---------------------------------------------------------------------------

/// Emit an `approval-requested` event, await the user's decision, and either
/// proceed (Allow{Once,Session}) or produce a `DenyResult` (explicit Deny or
/// fail-safe on dropped channel).
///
/// On `AllowSession`, persists the granted scope to the permissions DB and
/// refreshes the in-memory `granted` snapshot so subsequent requests in
/// this connection skip the prompt.
async fn await_user_approval(
    state: &Arc<AppState>,
    app: &AppHandle,
    granted: &Arc<RwLock<Vec<PersistedPermission>>>,
    server_id: &str,
    server_name: &str,
    action: &AgentAction,
    scope: &'static str,
) -> Result<(), DenyResult> {
    let approval_id = Ulid::new().to_string();
    let (tx, rx) = oneshot::channel::<ApprovalDecision>();
    state.approvals.register(approval_id.clone(), tx);

    let payload = ApprovalReqPayload {
        id: approval_id,
        server_id: server_id.to_string(),
        server_name: server_name.to_string(),
        tool: action.tool_name.clone(),
        kind: action.kind,
        target: action.target.clone(),
        scope: scope.to_string(),
        risk: risk_for_kind(action.kind, action.target.as_deref()),
        requested_at: Utc::now(),
    };
    let _ = app.emit(EVENT_APPROVAL_REQUESTED, &payload);

    match rx.await {
        Ok(ApprovalDecision::AllowOnce) => Ok(()),
        Ok(ApprovalDecision::AllowSession) => {
            // Persist + refresh in-memory snapshot. Failures are logged but
            // don't block the request — the user clicked "Always Allow", we
            // should honor that for at least the current call.
            let req_perm = RequestedPermission {
                scope: scope.to_string(),
                target: None,
                reason: Some(format!(
                    "User granted via runtime prompt ({})",
                    action.tool_name
                )),
            };
            if let Err(e) =
                permissions::grant_many(state.mcp.db(), server_id, &[req_perm])
            {
                tracing::warn!(error = %e, "failed to persist runtime grant");
            }
            if let Ok(updated) =
                permissions::list_for_server(state.mcp.db(), server_id)
            {
                *granted.write() = updated;
            }
            Ok(())
        }
        Ok(ApprovalDecision::Deny) => Err(DenyResult {
            scope,
            reason: format!("User denied {} via runtime prompt", action.tool_name),
        }),
        Err(_) => Err(DenyResult {
            scope,
            reason: "Approval prompt closed without response".to_string(),
        }),
    }
}
