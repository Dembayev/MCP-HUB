//! SQLite persistence layer.
//!
//! Stores the registry of installed MCP servers, runtime logs, and granted
//! permissions. We use `rusqlite` with the `bundled` feature so the binary
//! ships its own libsqlite3 — no external dependency on the user's system.

pub mod models;
pub mod permissions;

use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{Connection, OpenFlags};

use crate::error::AppResult;

/// Thread-safe wrapper around a single SQLite connection. For MCP Hub's
/// expected workload (single-user desktop, low write volume) a Mutex over one
/// connection is simpler and faster than a pool.
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> AppResult<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )?;

        // Pragmas tuned for desktop reliability over raw throughput.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn run_migrations(&self) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute_batch(SCHEMA_V1)?;
        Ok(())
    }

    /// Execute a closure with exclusive access to the connection.
    pub fn with_conn<R>(&self, f: impl FnOnce(&Connection) -> AppResult<R>) -> AppResult<R> {
        let conn = self.conn.lock();
        f(&conn)
    }
}

const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS servers (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    command         TEXT NOT NULL,
    args            TEXT NOT NULL DEFAULT '[]',   -- JSON array
    env             TEXT NOT NULL DEFAULT '{}',   -- JSON object
    transport       TEXT NOT NULL DEFAULT 'stdio',
    status          TEXT NOT NULL DEFAULT 'stopped',
    installed_at    TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    version         TEXT,
    source          TEXT,                          -- registry, manual, url
    icon_url        TEXT
);

CREATE INDEX IF NOT EXISTS idx_servers_status ON servers(status);

CREATE TABLE IF NOT EXISTS server_logs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id       TEXT NOT NULL,
    level           TEXT NOT NULL,
    message         TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_logs_server_created
    ON server_logs(server_id, created_at DESC);

CREATE TABLE IF NOT EXISTS permissions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id       TEXT NOT NULL,
    scope           TEXT NOT NULL,                 -- e.g. fs.read, net.outbound
    target          TEXT,                          -- path glob or host pattern
    granted         INTEGER NOT NULL DEFAULT 0,    -- boolean
    granted_at      TEXT,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE,
    UNIQUE (server_id, scope, target)
);
"#;
