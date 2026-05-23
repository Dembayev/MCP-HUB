//! Persistent registry of installed MCP servers. Thin wrapper over the SQLite
//! `servers` table.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use rusqlite::{params, Row};
use uuid::Uuid;

use crate::db::models::{InstallServerRequest, McpServer, ServerStatus, Transport};
use crate::db::Database;
use crate::error::{AppError, AppResult};

pub struct ServerRegistry {
    db: Arc<Database>,
}

impl ServerRegistry {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn list(&self) -> AppResult<Vec<McpServer>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, description, command, args, env, transport, status,
                        installed_at, updated_at, version, source, icon_url
                 FROM servers ORDER BY name ASC",
            )?;
            let rows = stmt
                .query_map([], row_to_server)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    pub fn get(&self, id: &str) -> AppResult<McpServer> {
        self.db.with_conn(|conn| {
            conn.query_row(
                "SELECT id, name, description, command, args, env, transport, status,
                        installed_at, updated_at, version, source, icon_url
                 FROM servers WHERE id = ?1",
                params![id],
                row_to_server,
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => AppError::ServerNotFound(id.to_string()),
                other => other.into(),
            })
        })
    }

    pub fn install(&self, req: InstallServerRequest) -> AppResult<McpServer> {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        let permissions = req.permissions.clone();

        let server = McpServer {
            id: id.clone(),
            name: req.name,
            description: req.description,
            command: req.command,
            args: req.args,
            env: req.env,
            transport: req.transport,
            status: ServerStatus::Stopped,
            installed_at: now,
            updated_at: now,
            version: req.version,
            source: req.source,
            icon_url: req.icon_url,
        };

        if server.name.trim().is_empty() {
            return Err(AppError::InvalidManifest("name must not be empty".into()));
        }
        if server.command.trim().is_empty() {
            return Err(AppError::InvalidManifest("command must not be empty".into()));
        }

        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO servers (id, name, description, command, args, env, transport,
                                      status, installed_at, updated_at, version, source, icon_url)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    server.id,
                    server.name,
                    server.description,
                    server.command,
                    serde_json::to_string(&server.args).unwrap_or_else(|_| "[]".into()),
                    serde_json::to_string(&server.env).unwrap_or_else(|_| "{}".into()),
                    server.transport.as_str(),
                    server.status.as_str(),
                    server.installed_at.to_rfc3339(),
                    server.updated_at.to_rfc3339(),
                    server.version,
                    server.source,
                    server.icon_url,
                ],
            )?;
            Ok(())
        })?;

        // Persist consented permissions so the sandbox layer can read them.
        if !permissions.is_empty() {
            crate::db::permissions::grant_many(&self.db, &server.id, &permissions)?;
        }

        tracing::info!(
            server_id = %server.id,
            name = %server.name,
            granted = permissions.len(),
            "installed server",
        );
        Ok(server)
    }

    pub fn set_status(&self, id: &str, status: ServerStatus) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let affected = self.db.with_conn(|conn| {
            Ok(conn.execute(
                "UPDATE servers SET status = ?1, updated_at = ?2 WHERE id = ?3",
                params![status.as_str(), now, id],
            )?)
        })?;

        if affected == 0 {
            return Err(AppError::ServerNotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn remove(&self, id: &str) -> AppResult<()> {
        let affected = self.db.with_conn(|conn| {
            Ok(conn.execute("DELETE FROM servers WHERE id = ?1", params![id])?)
        })?;

        if affected == 0 {
            return Err(AppError::ServerNotFound(id.to_string()));
        }
        Ok(())
    }
}

fn row_to_server(row: &Row<'_>) -> rusqlite::Result<McpServer> {
    let args_json: String = row.get(4)?;
    let env_json: String = row.get(5)?;
    let transport_str: String = row.get(6)?;
    let status_str: String = row.get(7)?;
    let installed_at_str: String = row.get(8)?;
    let updated_at_str: String = row.get(9)?;

    let args = serde_json::from_str::<Vec<String>>(&args_json).unwrap_or_default();
    let env =
        serde_json::from_str::<HashMap<String, String>>(&env_json).unwrap_or_default();

    Ok(McpServer {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        command: row.get(3)?,
        args,
        env,
        transport: Transport::from_str(&transport_str),
        status: ServerStatus::from_str(&status_str),
        installed_at: chrono::DateTime::parse_from_rfc3339(&installed_at_str)
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now()),
        updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now()),
        version: row.get(10)?,
        source: row.get(11)?,
        icon_url: row.get(12)?,
    })
}
