//! Per-server log capture.
//!
//! When an MCP server is started, its stdout and stderr are piped into
//! background tokio tasks that push every line into the `LogSink` AND emit
//! a Tauri `server-log` event for the React UI.
//!
//! The sink is a fixed-size ring buffer per server — designed for a live
//! tail view, not for archival. Persisting historical logs is deferred to
//! the SQLite `server_logs` table (not wired in yet — see roadmap).

use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub server_id: String,
    pub stream: LogStream,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

impl LogEntry {
    pub fn new(server_id: String, stream: LogStream, message: String) -> Self {
        Self {
            server_id,
            stream,
            message,
            timestamp: Utc::now(),
        }
    }
}

/// Bounded per-server log buffer. Drops oldest entries when full.
pub struct LogSink {
    buffers: Mutex<HashMap<String, VecDeque<LogEntry>>>,
    capacity_per_server: usize,
}

impl LogSink {
    pub fn new(capacity_per_server: usize) -> Self {
        Self {
            buffers: Mutex::new(HashMap::new()),
            capacity_per_server,
        }
    }

    pub fn push(&self, entry: &LogEntry) {
        let mut map = self.buffers.lock();
        let q = map
            .entry(entry.server_id.clone())
            .or_insert_with(|| VecDeque::with_capacity(self.capacity_per_server));
        if q.len() >= self.capacity_per_server {
            q.pop_front();
        }
        q.push_back(entry.clone());
    }

    /// Return the last `limit` entries for a server, oldest-first.
    pub fn snapshot(&self, server_id: &str, limit: usize) -> Vec<LogEntry> {
        let map = self.buffers.lock();
        match map.get(server_id) {
            Some(q) => {
                let start = q.len().saturating_sub(limit);
                q.iter().skip(start).cloned().collect()
            }
            None => Vec::new(),
        }
    }

    pub fn clear(&self, server_id: &str) {
        self.buffers.lock().remove(server_id);
    }
}
