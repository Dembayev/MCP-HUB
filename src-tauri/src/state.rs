//! Shared application state managed by Tauri.

use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::AppHandle;

use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::mcp::McpManager;

/// Container for all long-lived application services. Cloned cheaply via `Arc`.
pub struct AppState {
    pub db: Arc<Database>,
    pub mcp: Arc<McpManager>,
    pub data_dir: PathBuf,
    /// Per-session NDJSON traces. One file per session, named `<ulid>.ndjson`.
    /// Populated by the proxy (step 4) and the demo-seed command.
    pub sessions_dir: PathBuf,
    // Reserved for future use (e.g. user preferences cache).
    _settings: Mutex<()>,
}

impl AppState {
    /// Initialize the application state. Must be called from inside the
    /// Tauri `setup` callback so we can hand the `AppHandle` to services
    /// that emit events (notably the MCP manager).
    pub fn initialize(app_handle: AppHandle) -> AppResult<Self> {
        let data_dir = resolve_data_dir()?;
        std::fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join("mcp_hub.db");
        let db = Arc::new(Database::open(&db_path)?);
        db.run_migrations()?;
        crate::db::permissions::ensure_columns(&db)?;

        let profiles_dir = data_dir.join("sandbox-profiles");
        std::fs::create_dir_all(&profiles_dir)?;

        let sessions_dir = data_dir.join("sessions");
        std::fs::create_dir_all(&sessions_dir)?;

        let mcp = Arc::new(McpManager::new(db.clone(), profiles_dir, app_handle));

        tracing::info!(?data_dir, ?sessions_dir, "MCP Hub state initialized");

        Ok(Self {
            db,
            mcp,
            data_dir,
            sessions_dir,
            _settings: Mutex::new(()),
        })
    }
}

fn resolve_data_dir() -> AppResult<PathBuf> {
    dirs::data_dir()
        .map(|p| p.join("MCP Hub"))
        .ok_or_else(|| AppError::Internal("could not resolve OS data directory".into()))
}
