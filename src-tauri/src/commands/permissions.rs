//! Permission management commands surfaced to the React frontend.

use std::sync::Arc;

use tauri::State;

use crate::db::permissions::{self, PersistedPermission};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub async fn list_server_permissions(
    state: State<'_, Arc<AppState>>,
    server_id: String,
) -> AppResult<Vec<PersistedPermission>> {
    permissions::list_for_server(&state.db, &server_id)
}

#[tauri::command]
pub async fn revoke_permission(
    state: State<'_, Arc<AppState>>,
    id: i64,
) -> AppResult<()> {
    permissions::set_granted(&state.db, id, false)
}

#[tauri::command]
pub async fn grant_permission(
    state: State<'_, Arc<AppState>>,
    id: i64,
) -> AppResult<()> {
    permissions::set_granted(&state.db, id, true)
}
