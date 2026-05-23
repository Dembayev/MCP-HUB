//! Tauri IPC surface for the runtime approval flow.
//!
//! The frontend's approval modal calls [`resolve_approval`] with the user's
//! choice (Allow Once / Always Allow / Deny). This module looks up the
//! pending approval in [`crate::security::ApprovalRegistry`] and delivers
//! the decision through the oneshot the proxy is awaiting.

use std::sync::Arc;

use tauri::State;

use crate::error::{AppError, AppResult};
use crate::security::ApprovalDecision;
use crate::state::AppState;

/// Tauri event name emitted to the frontend when a new approval is pending.
pub const EVENT_APPROVAL_REQUESTED: &str = "approval-requested";

/// Deliver the user's decision for a pending approval. The proxy await-er
/// will receive it on the corresponding oneshot and proceed accordingly.
///
/// Returns an error only when the approval id is unknown — every active
/// approval should be resolvable. UI is expected to call this in response
/// to one of its three buttons; clicking the same prompt twice is a noop.
#[tauri::command]
pub async fn resolve_approval(
    state: State<'_, Arc<AppState>>,
    id: String,
    decision: ApprovalDecision,
) -> AppResult<()> {
    if state.approvals.resolve(&id, decision) {
        Ok(())
    } else {
        Err(AppError::Internal(format!(
            "no pending approval with id {id} (already resolved?)"
        )))
    }
}

/// Diagnostic / observability: how many approvals are currently waiting
/// for the user. The UI uses this to badge the modal queue.
#[tauri::command]
pub async fn pending_approval_count(state: State<'_, Arc<AppState>>) -> AppResult<usize> {
    Ok(state.approvals.pending_count())
}
