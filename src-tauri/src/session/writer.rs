//! Append-only NDJSON writer — the **primary storage layer** for sessions.
//!
//! Why append-only NDJSON (instead of an in-memory `SessionFile` we serialize
//! at the end):
//!
//! 1. **Crash safety.** Every fsync boundary is a recovery point. If the
//!    process dies, the file on disk is still a valid (truncated) session
//!    that the reader handles without special-casing.
//! 2. **Unified runtime + storage model.** The Timeline UI tails this same
//!    file. There is no "live state vs. persisted state" duality to keep
//!    in sync.
//! 3. **JSON export is a fold.** `session.json` is `read_ndjson() → reshape →
//!    write`. Trivial. Tested. No parallel writer to maintain.
//!
//! ## Concurrency
//!
//! `SessionWriter` is `Send` but not internally synchronized. Each session
//! has exactly one writer, owned by the supervising proxy task. If you need
//! to share, wrap in your runtime's preferred mutex.
//!
//! ## Fsync strategy
//!
//! Per-record fsync would be safe but slow; never-fsync would be fast but
//! lossy on crash. We batch: every `FSYNC_BATCH_SIZE` actions (configurable
//! via [`SessionWriter::with_fsync_batch`]) we flush the BufWriter and
//! `fsync(2)` the underlying file. Worst-case loss on crash is ~`batch_size`
//! trailing actions, but the meta record and every previously-fsynced batch
//! survive.
//!
//! `finalize()` always flushes + fsyncs before closing.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use crate::error::{AppError, AppResult};

use super::types::{Action, NdjsonRecord, SessionMeta, Stats, SCHEMA_VERSION};

/// Default number of action records between fsync calls.
///
/// 16 is a deliberately small number: at typical MCP request rates (~10/s in
/// active use) this means an fsync roughly every 1–2 seconds, which is
/// imperceptible to the user but tight enough that a crash never loses more
/// than ~10 actions.
pub const FSYNC_BATCH_SIZE: usize = 16;

/// Append-only NDJSON writer for a single session.
pub struct SessionWriter {
    path: PathBuf,
    file: BufWriter<File>,
    pending: usize,
    batch_size: usize,
    finalized: bool,
}

impl SessionWriter {
    /// Create a new session file at `path` and write the `meta` record + fsync.
    ///
    /// Fails if the file already exists — sessions are immutable artifacts;
    /// rotating a session means creating a new file at a new path.
    pub fn create(path: impl AsRef<Path>, meta: SessionMeta) -> AppResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(AppError::from)?;

        let mut writer = Self {
            path,
            file: BufWriter::new(file),
            pending: 0,
            batch_size: FSYNC_BATCH_SIZE,
            finalized: false,
        };

        let meta_rec = NdjsonRecord::Meta {
            schema_version: SCHEMA_VERSION.to_string(),
            session: meta,
        };
        writer.write_record(&meta_rec)?;
        writer.flush_and_sync()?;

        Ok(writer)
    }

    /// Override the fsync batch size. Useful for tests that want
    /// per-record durability (`batch_size = 1`).
    pub fn with_fsync_batch(mut self, batch_size: usize) -> Self {
        // Guard against zero, which would mean "never fsync between flushes" —
        // not what callers ever want.
        self.batch_size = batch_size.max(1);
        self
    }

    /// Append one action. May or may not trigger an fsync depending on
    /// where we are in the current batch.
    pub fn append(&mut self, action: &Action) -> AppResult<()> {
        if self.finalized {
            return Err(AppError::Internal(
                "session writer used after finalize()".to_string(),
            ));
        }

        let rec = NdjsonRecord::Action {
            action: action.clone(),
        };
        self.write_record(&rec)?;

        self.pending += 1;
        if self.pending >= self.batch_size {
            self.flush_and_sync()?;
        }
        Ok(())
    }

    /// Flush the in-memory buffer and fsync the file to disk immediately.
    /// Resets the batch counter.
    pub fn flush(&mut self) -> AppResult<()> {
        self.flush_and_sync()
    }

    /// Write the `end` record with computed stats, flush, fsync, and close.
    ///
    /// Consumes the writer — a session can only be finalized once. After
    /// this returns, the file on disk is a complete v0.1.0 session.
    pub fn finalize(mut self, ended_at: DateTime<Utc>, stats: Stats) -> AppResult<PathBuf> {
        let rec = NdjsonRecord::End { ended_at, stats };
        self.write_record(&rec)?;
        self.flush_and_sync()?;
        self.finalized = true;
        Ok(self.path.clone())
    }

    /// Path of the file being written. Useful for the export-button UX, which
    /// can offer to "Reveal in Finder" / open the file directly.
    pub fn path(&self) -> &Path {
        &self.path
    }

    // ---- internals -------------------------------------------------------

    fn write_record(&mut self, rec: &NdjsonRecord) -> AppResult<()> {
        // Compact serialization (one record per line) is what makes the file
        // greppable and parseable by line. Pretty-printed records would break
        // the NDJSON contract.
        let line = serde_json::to_string(rec)
            .map_err(|e| AppError::Internal(format!("session serialize: {e}")))?;
        self.file.write_all(line.as_bytes())?;
        self.file.write_all(b"\n")?;
        Ok(())
    }

    fn flush_and_sync(&mut self) -> AppResult<()> {
        self.file.flush()?;
        // sync_all = fsync + metadata. We want this even on the action-batch
        // path because partial writes could leave us with a half-line that the
        // reader has to discard.
        self.file.get_ref().sync_all()?;
        self.pending = 0;
        Ok(())
    }
}

impl Drop for SessionWriter {
    fn drop(&mut self) {
        // Best-effort flush so abrupt drops don't lose buffered records.
        // We deliberately don't fsync here — if the caller didn't finalize(),
        // the on-disk file is treated as truncated by the reader anyway, and
        // fsync in Drop is a known footgun (panic-during-panic, etc.).
        if !self.finalized {
            if let Err(err) = self.file.flush() {
                tracing::warn!(
                    error = %err,
                    path = %self.path.display(),
                    "session writer drop: flush failed; trailing actions may be lost",
                );
            }
        }
    }
}
