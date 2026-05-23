//! Runtime process supervisor for MCP servers.
//!
//! Each running server is owned by a tokio "supervisor" task. The manager
//! keeps a kill-channel into that task so `stop()` can ask it to terminate
//! the child cleanly. stdout/stderr are piped into separate tasks that push
//! each line into the [`LogSink`] and emit a `server-log` Tauri event for
//! the React frontend.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::db::models::{McpServer, ServerStatus};
use crate::db::permissions;
use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::mcp::agent::{classify_jsonrpc, classify_sandbox_denial, ActionSink};
use crate::mcp::logs::{LogEntry, LogSink, LogStream};
use crate::mcp::registry::ServerRegistry;
use crate::security::{for_current_platform, Sandbox};

const LOG_BUFFER_CAPACITY: usize = 1000;
const ACTION_BUFFER_CAPACITY: usize = 500;
const EVENT_SERVER_LOG: &str = "server-log";
const EVENT_SERVER_STATUS: &str = "server-status-changed";
const EVENT_AGENT_ACTION: &str = "agent-action";

pub struct McpManager {
    registry: Arc<ServerRegistry>,
    db: Arc<Database>,
    children: Mutex<HashMap<String, RunningServer>>,
    logs: Arc<LogSink>,
    actions: Arc<ActionSink>,
    sandbox: Box<dyn Sandbox>,
    profiles_dir: PathBuf,
    app_handle: AppHandle,
}

/// Per-server runtime handle. Holding `kill_tx` lets us ask the supervisor
/// task to terminate its child gracefully without taking the child out of
/// async context.
struct RunningServer {
    kill_tx: mpsc::Sender<()>,
}

impl McpManager {
    pub fn new(db: Arc<Database>, profiles_dir: PathBuf, app_handle: AppHandle) -> Self {
        Self {
            registry: Arc::new(ServerRegistry::new(db.clone())),
            db,
            children: Mutex::new(HashMap::new()),
            logs: Arc::new(LogSink::new(LOG_BUFFER_CAPACITY)),
            actions: Arc::new(ActionSink::new(ACTION_BUFFER_CAPACITY)),
            sandbox: for_current_platform(),
            profiles_dir,
            app_handle,
        }
    }

    pub fn registry(&self) -> &ServerRegistry {
        &self.registry
    }

    pub fn registry_arc(&self) -> Arc<ServerRegistry> {
        self.registry.clone()
    }

    pub fn logs(&self) -> &LogSink {
        &self.logs
    }

    pub fn logs_arc(&self) -> Arc<LogSink> {
        self.logs.clone()
    }

    pub fn actions(&self) -> &ActionSink {
        &self.actions
    }

    pub fn actions_arc(&self) -> Arc<ActionSink> {
        self.actions.clone()
    }

    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }

    /// Spawn a dedicated child for a proxy connection. Unlike [`Self::start`]
    /// the resulting child is *not* parked in `self.children` — the caller
    /// (the proxy connection handler) owns its lifecycle, including stdio.
    pub fn spawn_for_proxy(&self, server_id: &str) -> AppResult<Child> {
        let server = self.registry.get(server_id)?;
        let resolved_args: Vec<String> = server.args.iter().map(|a| expand_env(a)).collect();
        let resolved_server = McpServer {
            args: resolved_args.clone(),
            ..server.clone()
        };

        let granted =
            permissions::list_for_server(&self.db, server_id).unwrap_or_default();
        let prepared = self
            .sandbox
            .prepare(&resolved_server, &granted, &self.profiles_dir)?;

        let mut command = Command::new(&prepared.program);
        command.args(&prepared.args);
        for (k, v) in &server.env {
            command.env(k, expand_env(v));
        }
        if let Some(home) = dirs::home_dir() {
            command.current_dir(home);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        tracing::info!(
            server = %server.name,
            enforcement = prepared.enforcement,
            "spawning MCP server for proxy",
        );

        command
            .spawn()
            .map_err(|e| AppError::Process(format!("proxy spawn failed: {}", e)))
    }

    /// Spawn the server's process and start tailing its output. No-op if
    /// already running.
    pub async fn start(&self, id: &str) -> AppResult<McpServer> {
        if self.children.lock().contains_key(id) {
            return self.registry.get(id);
        }

        let server = self.registry.get(id)?;
        self.registry.set_status(id, ServerStatus::Starting)?;
        self.emit_status(id, ServerStatus::Starting);

        // `tokio::process::Command` does not perform shell expansion, so we
        // resolve `~`, `$VAR` and `${VAR}` ourselves. Marketplace entries can
        // declare paths like `$HOME` and have them resolve at spawn time.
        let resolved_args: Vec<String> = server.args.iter().map(|a| expand_env(a)).collect();
        let resolved_server = McpServer {
            args: resolved_args.clone(),
            ..server.clone()
        };

        // Build a sandboxed launcher from the user's granted permissions.
        // On macOS we get `sandbox-exec -f <profile> <command> <args...>`;
        // on Linux/Windows we currently get the bare command back unchanged.
        let granted = permissions::list_for_server(&self.db, id).unwrap_or_default();
        let prepared = self
            .sandbox
            .prepare(&resolved_server, &granted, &self.profiles_dir)?;

        let mut command = Command::new(&prepared.program);
        command.args(&prepared.args);
        for (k, v) in &server.env {
            command.env(k, expand_env(v));
        }
        // Default cwd to the user's home so relative paths in MCP servers
        // don't anchor on the Tauri binary's working directory (which is
        // `src-tauri/` in dev and platform-dependent in release).
        if let Some(home) = dirs::home_dir() {
            command.current_dir(home);
        }
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        tracing::info!(
            server = %server.name,
            enforcement = prepared.enforcement,
            program = %prepared.program,
            args = ?prepared.args,
            "spawning MCP server",
        );

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(server = %server.name, error = %e, "spawn failed");
                let _ = self.registry.set_status(id, ServerStatus::Crashed);
                self.emit_status(id, ServerStatus::Crashed);
                return Err(AppError::Process(format!(
                    "failed to spawn {}: {}",
                    server.command, e
                )));
            }
        };

        if let Some(stdout) = child.stdout.take() {
            self.spawn_log_reader(stdout, id, LogStream::Stdout);
        }
        if let Some(stderr) = child.stderr.take() {
            self.spawn_log_reader(stderr, id, LogStream::Stderr);
        }

        let (kill_tx, kill_rx) = mpsc::channel::<()>(1);
        self.spawn_supervisor(child, id, kill_rx);

        self.children
            .lock()
            .insert(id.to_string(), RunningServer { kill_tx });
        self.registry.set_status(id, ServerStatus::Running)?;
        self.emit_status(id, ServerStatus::Running);

        tracing::info!(server = %server.name, "server started");
        self.registry.get(id)
    }

    /// Signal the supervisor to terminate. The supervisor will update the
    /// DB status and emit the final event when the child actually exits.
    pub async fn stop(&self, id: &str) -> AppResult<McpServer> {
        if let Some(running) = self.children.lock().remove(id) {
            let _ = running.kill_tx.try_send(());
        }
        // Optimistic status update for snappy UX. Supervisor will overwrite
        // if it produced a different exit (e.g. crashed during shutdown).
        self.registry.set_status(id, ServerStatus::Stopped)?;
        self.emit_status(id, ServerStatus::Stopped);
        self.registry.get(id)
    }

    /// Stop (if running), drop logs and actions, then remove from the registry.
    pub async fn remove(&self, id: &str) -> AppResult<()> {
        let _ = self.stop(id).await;
        self.logs.clear(id);
        self.actions.clear(id);
        self.registry.remove(id)
    }

    fn emit_status(&self, id: &str, status: ServerStatus) {
        let payload = serde_json::json!({ "id": id, "status": status.as_str() });
        if let Err(err) = self.app_handle.emit(EVENT_SERVER_STATUS, payload) {
            tracing::warn!(error = %err, "failed to emit server-status-changed");
        }
    }

    fn spawn_log_reader<R>(&self, reader: R, server_id: &str, stream: LogStream)
    where
        R: AsyncRead + Unpin + Send + 'static,
    {
        let log_sink = self.logs.clone();
        let action_sink = self.actions.clone();
        let app = self.app_handle.clone();
        let server_id = server_id.to_string();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        // Always push to the log buffer + emit the log event.
                        let entry = LogEntry::new(server_id.clone(), stream, line.clone());
                        log_sink.push(&entry);
                        let _ = app.emit(EVENT_SERVER_LOG, &entry);

                        // Opportunistically classify as an agent action.
                        // stdout JSON-RPC produces success-status cards;
                        // stderr lines matching sandbox/permission denial
                        // markers produce a denied card. Either way, the
                        // Timeline becomes the single source of truth.
                        let action = match stream {
                            LogStream::Stdout => classify_jsonrpc(&line, &server_id),
                            LogStream::Stderr => classify_sandbox_denial(&line, &server_id),
                        };
                        if let Some(action) = action {
                            action_sink.push(&action);
                            let _ = app.emit(EVENT_AGENT_ACTION, &action);
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        tracing::warn!(server_id = %server_id, error = %err, "log reader error");
                        break;
                    }
                }
            }
        });
    }

    fn spawn_supervisor(
        &self,
        mut child: Child,
        server_id: &str,
        mut kill_rx: mpsc::Receiver<()>,
    ) {
        let registry = self.registry.clone();
        let app = self.app_handle.clone();
        let server_id = server_id.to_string();
        tokio::spawn(async move {
            let exit = tokio::select! {
                s = child.wait() => s,
                _ = kill_rx.recv() => {
                    let _ = child.start_kill();
                    child.wait().await
                }
            };

            let final_status = match exit {
                Ok(s) if s.success() => ServerStatus::Stopped,
                Ok(s) => {
                    tracing::info!(server_id = %server_id, code = ?s.code(), "server exited non-zero");
                    ServerStatus::Crashed
                }
                Err(err) => {
                    tracing::warn!(server_id = %server_id, error = %err, "wait() failed");
                    ServerStatus::Crashed
                }
            };

            if let Err(err) = registry.set_status(&server_id, final_status) {
                tracing::warn!(error = %err, "failed to persist final status");
            }
            let payload = serde_json::json!({ "id": server_id, "status": final_status.as_str() });
            let _ = app.emit(EVENT_SERVER_STATUS, payload);
        });
    }
}

/// Resolve `~`, `$VAR`, and `${VAR}` in a string against the current process
/// environment. Unknown variables expand to an empty string (matching POSIX
/// shell behavior). Malformed `${` without a closing `}` is left literal.
///
/// We deliberately do NOT invoke a shell — that would open us up to injection
/// from marketplace data. This is a tiny, well-defined subset of shell
/// expansion, and only that.
fn expand_env(input: &str) -> String {
    // 1. `~` prefix (`~` alone or `~/...`).
    let with_tilde = match dirs::home_dir() {
        Some(home) if input == "~" => return home.to_string_lossy().into_owned(),
        Some(home) if input.starts_with("~/") => {
            format!("{}/{}", home.display(), &input[2..])
        }
        _ => input.to_string(),
    };

    // 2. `$VAR` and `${VAR}`.
    let mut out = String::with_capacity(with_tilde.len());
    let mut chars = with_tilde.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('{') => {
                chars.next(); // consume '{'
                let mut name = String::new();
                let mut closed = false;
                while let Some(&ch) = chars.peek() {
                    if ch == '}' {
                        chars.next();
                        closed = true;
                        break;
                    }
                    name.push(ch);
                    chars.next();
                }
                if closed {
                    if let Ok(value) = std::env::var(&name) {
                        out.push_str(&value);
                    }
                } else {
                    // Malformed `${` — leave it literal so the user can debug.
                    out.push('$');
                    out.push('{');
                    out.push_str(&name);
                }
            }
            Some(ch) if ch.is_ascii_alphabetic() || ch == '_' => {
                let mut name = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_ascii_alphanumeric() || ch == '_' {
                        name.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if let Ok(value) = std::env::var(&name) {
                    out.push_str(&value);
                }
            }
            _ => out.push('$'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::expand_env;

    #[test]
    fn passes_through_plain_strings() {
        assert_eq!(expand_env("hello"), "hello");
        assert_eq!(expand_env(""), "");
        assert_eq!(expand_env("a/b/c"), "a/b/c");
    }

    #[test]
    fn resolves_named_var() {
        std::env::set_var("MCP_HUB_TEST_VAR", "value42");
        assert_eq!(expand_env("$MCP_HUB_TEST_VAR"), "value42");
        assert_eq!(expand_env("x=$MCP_HUB_TEST_VAR;"), "x=value42;");
        assert_eq!(expand_env("${MCP_HUB_TEST_VAR}_suffix"), "value42_suffix");
    }

    #[test]
    fn unset_var_expands_to_empty() {
        std::env::remove_var("MCP_HUB_TEST_MISSING");
        assert_eq!(expand_env("$MCP_HUB_TEST_MISSING"), "");
        assert_eq!(expand_env("a${MCP_HUB_TEST_MISSING}b"), "ab");
    }

    #[test]
    fn dangling_dollar_left_literal() {
        assert_eq!(expand_env("$"), "$");
        assert_eq!(expand_env("$1"), "$1"); // numeric — not a var name
        assert_eq!(expand_env("${unclosed"), "${unclosed");
    }

    #[test]
    fn tilde_prefix_expands_to_home() {
        if let Some(home) = dirs::home_dir() {
            let expected = home.display().to_string();
            assert_eq!(expand_env("~"), expected);
            assert_eq!(expand_env("~/code"), format!("{}/code", expected));
            // tilde only counts at the start.
            assert_eq!(expand_env("a/~/b"), "a/~/b");
        }
    }
}
