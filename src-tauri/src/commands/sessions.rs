//! Session trace commands — Timeline tab's IPC surface.
//!
//! The frontend uses three commands:
//!
//! - [`list_sessions`] — scan `<data_dir>/sessions/` and return summary
//!   metadata for each `*.ndjson` file. Polled by the UI ~1×/s.
//! - [`get_session`] — return the full [`SessionFile`] for one id, with
//!   actions sorted by `seq` (reader handles that).
//! - [`get_session_path`] — return the absolute filesystem path of a session,
//!   useful for "Reveal in Finder" / "Copy path" UI affordances.
//!
//! Plus a small dev-affordance command:
//!
//! - [`seed_demo_session`] — write a canonical demo trace (start → allowed
//!   read → denied `~/.ssh` write) so the Timeline tab is testable BEFORE
//!   the proxy is instrumented in step 4.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::session::demo::make_demo_session;
use crate::session::reader::{read_ndjson, ReadOutcome};
use crate::session::writer::SessionWriter;
use crate::session::SessionFile;
use crate::state::AppState;

/// Sidebar-ready summary of a session. Cheap to compute: only the meta
/// record + a count are needed for the list; full action data is fetched on
/// selection via [`get_session`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    /// ULID as a string (frontend never parses ULIDs).
    pub id: String,
    pub server_id: String,
    pub server_name: String,
    pub client_name: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub action_count: u64,
    pub denied_count: u64,
    pub error_count: u64,
    pub duration_ms: u64,
    /// `"complete" | "truncated"` — matches the reader's `ReadOutcome`. Used
    /// by the UI to show "session in progress" vs "session ended" badges.
    pub status: &'static str,
    /// Absolute path on disk. Exposed for "Reveal in Finder" UI.
    pub path: String,
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, Arc<AppState>>) -> AppResult<Vec<SessionSummary>> {
    let dir = state.sessions_dir.clone();
    // File I/O off the runtime thread.
    tokio::task::spawn_blocking(move || list_sessions_blocking(&dir))
        .await
        .map_err(|e| AppError::Internal(format!("list_sessions join: {e}")))?
}

#[tauri::command]
pub async fn get_session(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<SessionFile> {
    let path = state.sessions_dir.join(format!("{id}.ndjson"));
    tokio::task::spawn_blocking(move || {
        let outcome = read_ndjson(&path)?;
        Ok::<_, AppError>(outcome.into_file())
    })
    .await
    .map_err(|e| AppError::Internal(format!("get_session join: {e}")))?
}

#[tauri::command]
pub async fn get_session_path(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<String> {
    let path = state.sessions_dir.join(format!("{id}.ndjson"));
    if !path.exists() {
        return Err(AppError::Internal(format!("no session with id {id}")));
    }
    Ok(path.to_string_lossy().into_owned())
}

/// Build a fresh demo session and write it to disk. Returns the generated
/// session id so the UI can auto-select it.
#[tauri::command]
pub async fn seed_demo_session(state: State<'_, Arc<AppState>>) -> AppResult<String> {
    let dir = state.sessions_dir.clone();
    tokio::task::spawn_blocking(move || seed_demo_session_blocking(&dir))
        .await
        .map_err(|e| AppError::Internal(format!("seed_demo_session join: {e}")))?
}

// ---------------------------------------------------------------------------
// Internals (sync, run inside spawn_blocking)
// ---------------------------------------------------------------------------

fn list_sessions_blocking(dir: &std::path::Path) -> AppResult<Vec<SessionSummary>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut summaries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("ndjson") {
            continue;
        }

        // Skip files that fail to parse — log and continue. A single bad
        // session shouldn't break the whole list.
        let outcome = match read_ndjson(&path) {
            Ok(o) => o,
            Err(err) => {
                tracing::warn!(?path, error = %err, "skipping unreadable session");
                continue;
            }
        };
        let status = if outcome.is_truncated() {
            "truncated"
        } else {
            "complete"
        };
        let file = match outcome {
            ReadOutcome::Complete(f) | ReadOutcome::Truncated(f) => f,
        };

        let stats = file.stats.as_ref();
        summaries.push(SessionSummary {
            id: file.session.id.to_string(),
            server_id: file.session.server.id.clone(),
            server_name: file.session.server.name.clone(),
            client_name: file.session.client.name.clone(),
            started_at: file.session.started_at,
            ended_at: file.session.ended_at,
            action_count: stats.map(|s| s.total_actions).unwrap_or(file.actions.len() as u64),
            denied_count: stats.map(|s| s.denied_count).unwrap_or(0),
            error_count: stats.map(|s| s.error_count).unwrap_or(0),
            duration_ms: stats.map(|s| s.duration_ms).unwrap_or(0),
            status,
            path: path.to_string_lossy().into_owned(),
        });
    }

    // Most recent first — natural "newest at top" UI ordering.
    summaries.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(summaries)
}

fn seed_demo_session_blocking(dir: &std::path::Path) -> AppResult<String> {
    std::fs::create_dir_all(dir)?;
    let file = make_demo_session();
    let session_id = file.session.id.to_string();
    let path = dir.join(format!("{session_id}.ndjson"));

    let meta = file.session.clone();
    let mut writer = SessionWriter::create(&path, meta)?;
    for action in &file.actions {
        writer.append(action)?;
    }
    let ended_at = file.session.ended_at.unwrap_or_else(Utc::now);
    let stats = file.stats.unwrap_or_default();
    writer.finalize(ended_at, stats)?;

    Ok(session_id)
}
