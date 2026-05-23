//! MCP Hub — desktop application for managing MCP servers and AI agents.
//!
//! Architecture:
//! - `db`        — SQLite persistence layer (server registry, logs, permissions).
//! - `mcp`       — process manager that spawns and supervises MCP servers.
//! - `security`  — permissions model and sandbox policy.
//! - `session`   — append-only NDJSON trace primitive (see docs/SESSION_SCHEMA.md).
//! - `commands`  — Tauri commands exposed to the React frontend.
//! - `state`     — shared application state, managed by Tauri.

pub mod commands;
pub mod db;
pub mod error;
pub mod mcp;
pub mod security;
pub mod session;
pub mod state;

use std::sync::Arc;

use tauri::Manager;
use tracing_subscriber::EnvFilter;

use crate::state::AppState;

/// Entry point invoked from `main.rs`. Sets up logging and the Tauri runtime.
/// AppState is constructed inside the `setup` callback so the MCP manager can
/// hold an `AppHandle` to emit events back to the frontend.
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = Arc::new(AppState::initialize(handle.clone())?);

            // Spawn the proxy listener as a background tokio task. It
            // accepts connections from `mcp-hub-proxy` workers spawned by
            // MCP clients (Claude Desktop, Cursor, …) and pumps JSON-RPC
            // through with classification + permission gating.
            let proxy_sock = state.data_dir.join("proxy.sock");
            let proxy_state = state.clone();
            let proxy_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = mcp::proxy::run_listener(proxy_sock, proxy_state, proxy_handle)
                    .await
                {
                    tracing::error!(error = %err, "proxy listener crashed");
                }
            });

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::servers::list_servers,
            commands::servers::install_server,
            commands::servers::start_server,
            commands::servers::stop_server,
            commands::servers::remove_server,
            commands::servers::get_server,
            commands::servers::get_server_logs,
            commands::servers::clear_server_logs,
            commands::servers::get_server_actions,
            commands::servers::clear_server_actions,
            commands::permissions::list_server_permissions,
            commands::permissions::grant_permission,
            commands::permissions::revoke_permission,
            commands::approvals::resolve_approval,
            commands::approvals::pending_approval_count,
            commands::sessions::list_sessions,
            commands::sessions::get_session,
            commands::sessions::get_session_path,
            commands::sessions::seed_demo_session,
            commands::system::app_info,
            commands::system::get_proxy_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MCP Hub");
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,mcp_hub_lib=debug"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}
