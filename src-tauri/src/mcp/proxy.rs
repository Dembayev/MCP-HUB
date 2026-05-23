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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
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

    let client_to_server = async move {
        while let Some(line) = sock_lines.next_line().await? {
            // Mirror to LogSink so the Activity view shows exactly what the
            // client sent, even if we end up denying the forward.
            let entry = LogEntry::new(server_id_for_req.clone(), LogStream::Stdout, line.clone());
            logs_for_req.push(&entry);

            match classify_jsonrpc(&line, &server_id_for_req) {
                Some(mut action) => {
                    let rpc_id = action.request_id.clone();

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
                                        started: Instant::now(),
                                    },
                                );
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
    let mut child_stdout_lines = BufReader::new(child_stdout).lines();

    let server_to_client = async move {
        while let Some(line) = child_stdout_lines.next_line().await? {
            let entry =
                LogEntry::new(server_id_for_resp.clone(), LogStream::Stdout, line.clone());
            logs_for_resp.push(&entry);

            if let Some(updated) = match_response(&line, &pending_for_resp) {
                actions_for_resp.push_or_update(&updated);
                let _ = app_for_resp.emit(EVENT_AGENT_ACTION, &updated);
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
    tracing::info!(server = %server.name, "proxy connection closed");
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type PendingMap = Arc<Mutex<HashMap<String, InFlight>>>;

struct InFlight {
    action: AgentAction,
    started: Instant,
}

fn rpc_id_key(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn match_response(line: &str, pending: &PendingMap) -> Option<AgentAction> {
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

    let latency_ms = in_flight.started.elapsed().as_millis() as u64;
    let mut action = in_flight.action;
    action.latency_ms = Some(latency_ms);
    action.timestamp = Utc::now();

    if let Some(error) = value.get("error") {
        action.status = ActionStatus::Failed;
        action.error = Some(
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("server error")
                .to_string(),
        );
    } else {
        action.status = ActionStatus::Success;
    }
    Some(action)
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
