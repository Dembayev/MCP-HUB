//! MCP server CRUD + lifecycle commands.

use std::sync::Arc;

use tauri::State;

use crate::db::models::{InstallServerRequest, McpServer};
use crate::error::AppResult;
use crate::mcp::{AgentAction, LogEntry};
use crate::state::AppState;

#[tauri::command]
pub async fn list_servers(state: State<'_, Arc<AppState>>) -> AppResult<Vec<McpServer>> {
    state.mcp.registry().list()
}

#[tauri::command]
pub async fn get_server(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<McpServer> {
    state.mcp.registry().get(&id)
}

#[tauri::command]
pub async fn install_server(
    state: State<'_, Arc<AppState>>,
    request: InstallServerRequest,
) -> AppResult<McpServer> {
    state.mcp.registry().install(request)
}

#[tauri::command]
pub async fn start_server(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<McpServer> {
    state.mcp.start(&id).await
}

#[tauri::command]
pub async fn stop_server(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<McpServer> {
    state.mcp.stop(&id).await
}

#[tauri::command]
pub async fn remove_server(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<()> {
    state.mcp.remove(&id).await
}

#[tauri::command]
pub async fn get_server_logs(
    state: State<'_, Arc<AppState>>,
    id: String,
    limit: Option<usize>,
) -> AppResult<Vec<LogEntry>> {
    Ok(state.mcp.logs().snapshot(&id, limit.unwrap_or(500)))
}

#[tauri::command]
pub async fn clear_server_logs(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<()> {
    state.mcp.logs().clear(&id);
    Ok(())
}

#[tauri::command]
pub async fn get_server_actions(
    state: State<'_, Arc<AppState>>,
    id: String,
    limit: Option<usize>,
) -> AppResult<Vec<AgentAction>> {
    Ok(state.mcp.actions().snapshot(&id, limit.unwrap_or(200)))
}

#[tauri::command]
pub async fn clear_server_actions(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<()> {
    state.mcp.actions().clear(&id);
    Ok(())
}
