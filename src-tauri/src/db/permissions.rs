//! Persisted permission grants per server.
//!
//! Permissions describe what a server *is allowed to do* — the trust
//! contract surfaced to the user. Granting happens during install (when the
//! user clicks Allow in the install dialog) and can be revoked later from
//! the Permissions page. The sandbox layer consumes the granted set to
//! build platform-specific enforcement (sandbox-exec profiles on macOS,
//! AppArmor / job objects later).
//!
//! Note: the schema is created in [`crate::db`] (CREATE TABLE IF NOT
//! EXISTS); the `reason` column is added idempotently below for users
//! whose database predates that field.

use chrono::{DateTime, Utc};
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedPermission {
    pub id: i64,
    pub server_id: String,
    pub scope: String,
    pub target: Option<String>,
    pub reason: Option<String>,
    pub granted: bool,
    pub granted_at: Option<DateTime<Utc>>,
}

/// Payload accepted alongside an `InstallServerRequest`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestedPermission {
    pub scope: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Run idempotent column adds for schemas that predate the current model.
/// SQLite has no `ADD COLUMN IF NOT EXISTS`, so we swallow the failure.
pub fn ensure_columns(db: &Database) -> AppResult<()> {
    db.with_conn(|conn| {
        // Errors are expected on already-up-to-date schemas; not fatal.
        let _ = conn.execute("ALTER TABLE permissions ADD COLUMN reason TEXT", []);
        Ok(())
    })
}

pub fn list_for_server(db: &Database, server_id: &str) -> AppResult<Vec<PersistedPermission>> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, server_id, scope, target, reason, granted, granted_at
             FROM permissions
             WHERE server_id = ?1
             ORDER BY scope ASC, target ASC",
        )?;
        let rows = stmt
            .query_map(params![server_id], row_to_permission)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

/// Insert (or upsert) a batch of permissions for a server. All requested
/// permissions are marked `granted = true` with the current timestamp —
/// the install dialog is the consent moment.
pub fn grant_many(
    db: &Database,
    server_id: &str,
    perms: &[RequestedPermission],
) -> AppResult<()> {
    if perms.is_empty() {
        return Ok(());
    }
    let now = Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        for p in perms {
            conn.execute(
                "INSERT INTO permissions (server_id, scope, target, reason, granted, granted_at)
                 VALUES (?1, ?2, ?3, ?4, 1, ?5)
                 ON CONFLICT(server_id, scope, target) DO UPDATE SET
                     reason = excluded.reason,
                     granted = 1,
                     granted_at = excluded.granted_at",
                params![server_id, p.scope, p.target, p.reason, now],
            )?;
        }
        Ok(())
    })
}

pub fn set_granted(db: &Database, id: i64, granted: bool) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE permissions
             SET granted = ?1, granted_at = ?2
             WHERE id = ?3",
            params![granted as i64, if granted { Some(now) } else { None }, id],
        )?;
        Ok(())
    })
}

fn row_to_permission(row: &Row<'_>) -> rusqlite::Result<PersistedPermission> {
    let granted_at_str: Option<String> = row.get(6)?;
    let granted_at = granted_at_str.and_then(|s| {
        chrono::DateTime::parse_from_rfc3339(&s)
            .ok()
            .map(|d| d.with_timezone(&chrono::Utc))
    });

    Ok(PersistedPermission {
        id: row.get(0)?,
        server_id: row.get(1)?,
        scope: row.get(2)?,
        target: row.get(3)?,
        reason: row.get(4)?,
        granted: row.get::<_, i64>(5)? != 0,
        granted_at,
    })
}
