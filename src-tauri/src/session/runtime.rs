//! Async runtime for session writing — the production-facing surface.
//!
//! Per `mcp_hub_launch_guardrails` (memory):
//!
//! - **No Mutex in the hot path.** Producers send [`Action`] records through
//!   an `mpsc::channel`; a dedicated blocking task owns the underlying
//!   [`SessionWriter`] exclusively and drains the channel serially.
//! - **`seq` is logical order; disk is arrival order.** The writer task
//!   writes in the order it receives messages. Downstream consumers go
//!   through [`super::read_ndjson`], which sorts by `seq` per spec §3.
//! - **Single source of truth for disk I/O.** Proxy code (and any future
//!   producer) never touches a file; it appends through a [`SessionAppender`].
//!
//! ## Architecture
//!
//! ```text
//!   proxy client→server pump  ─┐
//!                               │  SessionAppender (clone of mpsc::Sender)
//!   proxy server→client pump  ─┤    │
//!                               │    ▼
//!   lifecycle hook            ─┘  mpsc::channel
//!                                       │
//!                                       ▼
//!                              blocking writer task
//!                                       │ owns SessionWriter
//!                                       ▼
//!                                NDJSON file on disk
//! ```
//!
//! The writer task runs under `tokio::task::spawn_blocking` because the
//! underlying file I/O + fsync are blocking syscalls. Using
//! `Receiver::blocking_recv` inside the task is the supported way to
//! cross-bridge from async producers to a sync consumer.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use tokio::sync::{mpsc, oneshot};

use crate::error::{AppError, AppResult};

use super::types::{Action, SessionMeta, Stats};
use super::writer::SessionWriter;

/// Default channel capacity. At ~10 MCP requests/sec this is ~25 seconds of
/// in-flight buffer — far more than the writer needs to drain. Backpressure
/// kicks in on bursts faster than the writer can fsync.
pub const DEFAULT_CHANNEL_CAPACITY: usize = 256;

/// Messages the writer task accepts.
enum WriterMsg {
    Append(Action),
    Finalize {
        ended_at: DateTime<Utc>,
        stats: Stats,
        ack: oneshot::Sender<AppResult<PathBuf>>,
    },
}

/// Lifecycle handle for a session. Owns finalization; producers use
/// [`SessionHandle::appender`] to get a clonable [`SessionAppender`].
///
/// ## Ordering contract
///
/// Callers MUST drop all [`SessionAppender`] clones before calling
/// [`SessionHandle::finalize`]. Any `append` after `finalize` is sent races
/// the end record and may be silently dropped. In practice this is naturally
/// enforced by structuring producers as scoped tasks that hold their appender
/// for their lifetime; the supervising task awaits them, then finalizes.
pub struct SessionHandle {
    appender: SessionAppender,
}

/// Clonable, async-safe handle for sending [`Action`] records to the writer.
///
/// Pass this around freely — each clone shares the same underlying channel,
/// so all producers feed one writer task in arrival order.
#[derive(Clone)]
pub struct SessionAppender {
    tx: mpsc::Sender<WriterMsg>,
}

impl SessionHandle {
    /// Spawn the writer task and create the session file on disk.
    ///
    /// Returns once the writer has written the `meta` record and fsynced —
    /// i.e. the session is durable on disk before this call returns. If the
    /// file already exists, or the directory is unwritable, the error is
    /// surfaced here (not deferred to the first `append`).
    pub async fn spawn(path: PathBuf, meta: SessionMeta) -> AppResult<Self> {
        Self::spawn_with_capacity(path, meta, DEFAULT_CHANNEL_CAPACITY).await
    }

    /// Same as [`spawn`](Self::spawn) but lets callers tune the channel size
    /// (useful for tests that want to surface backpressure behavior).
    pub async fn spawn_with_capacity(
        path: PathBuf,
        meta: SessionMeta,
        capacity: usize,
    ) -> AppResult<Self> {
        let (tx, rx) = mpsc::channel::<WriterMsg>(capacity.max(1));
        let (ready_tx, ready_rx) = oneshot::channel::<Result<(), String>>();

        // The writer task owns the SessionWriter exclusively. spawn_blocking
        // is the right primitive: the writer's append/fsync are blocking
        // syscalls and we don't want to wedge a tokio worker.
        tokio::task::spawn_blocking(move || run_writer_task(path, meta, rx, ready_tx));

        // Wait for the writer to confirm the file was created + meta fsynced.
        // If the task died before sending (e.g. panic), surface a sensible
        // error rather than hanging on append later.
        match ready_rx.await {
            Ok(Ok(())) => Ok(Self {
                appender: SessionAppender { tx },
            }),
            Ok(Err(msg)) => Err(AppError::Internal(format!("session writer: {msg}"))),
            Err(_recv_err) => Err(AppError::Internal(
                "session writer task died before reporting ready".into(),
            )),
        }
    }

    /// Get a clonable appender. Pass this to producer tasks.
    pub fn appender(&self) -> SessionAppender {
        self.appender.clone()
    }

    /// Send `Finalize` and await the writer's ack. Consumes the handle.
    ///
    /// Drops the embedded appender immediately so that, combined with all
    /// external appenders being dropped by the caller, the channel can close
    /// and the writer task exits cleanly.
    pub async fn finalize(
        self,
        ended_at: DateTime<Utc>,
        stats: Stats,
    ) -> AppResult<PathBuf> {
        let (ack_tx, ack_rx) = oneshot::channel();
        let tx = self.appender.tx.clone();
        // Drop our owned appender first so the only remaining tx is the local
        // `tx` clone, which we'll drop after `send`. External appenders are
        // the caller's responsibility (see ordering contract).
        drop(self.appender);

        tx.send(WriterMsg::Finalize {
            ended_at,
            stats,
            ack: ack_tx,
        })
        .await
        .map_err(|_| AppError::Internal("session writer task is gone".into()))?;
        drop(tx);

        ack_rx
            .await
            .map_err(|_| AppError::Internal("session writer died during finalize".into()))?
    }
}

impl SessionAppender {
    /// Append one [`Action`] to the session. Awaits if the channel is full
    /// (backpressure). Returns an error only if the writer task has died.
    pub async fn append(&self, action: Action) -> AppResult<()> {
        self.tx
            .send(WriterMsg::Append(action))
            .await
            .map_err(|_| AppError::Internal("session writer task is gone".into()))
    }
}

/// Body of the writer task. Runs on a `spawn_blocking` thread.
///
/// Lifetime:
///
/// 1. Create the `SessionWriter` (writes meta + fsyncs).
/// 2. Signal ready (or signal create-failure) via `ready_tx`.
/// 3. `blocking_recv` until the channel closes or a `Finalize` arrives.
/// 4. On `Finalize`: call `writer.finalize`, send result via the embedded ack.
/// 5. On channel close without `Finalize`: drop the writer (best-effort flush,
///    no end record → the file is "truncated" per the reader's terminology).
fn run_writer_task(
    path: PathBuf,
    meta: SessionMeta,
    mut rx: mpsc::Receiver<WriterMsg>,
    ready_tx: oneshot::Sender<Result<(), String>>,
) {
    let mut writer = match SessionWriter::create(&path, meta) {
        Ok(w) => {
            // If the receiver was already dropped, no one cares — keep going
            // anyway; we'll exit on first recv attempt.
            let _ = ready_tx.send(Ok(()));
            w
        }
        Err(err) => {
            let _ = ready_tx.send(Err(err.to_string()));
            return;
        }
    };

    while let Some(msg) = rx.blocking_recv() {
        match msg {
            WriterMsg::Append(action) => {
                if let Err(err) = writer.append(&action) {
                    // We do NOT abort the loop on a single append failure —
                    // the producer can't know the writer crashed, and we want
                    // to give Finalize a chance to surface the error path. We
                    // log and continue; subsequent fsync errors will surface
                    // via finalize().
                    tracing::warn!(
                        seq = action.seq,
                        error = %err,
                        "session writer: append failed (continuing)"
                    );
                }
            }
            WriterMsg::Finalize {
                ended_at,
                stats,
                ack,
            } => {
                let result = writer.finalize(ended_at, stats);
                let _ = ack.send(result);
                return;
            }
        }
    }

    // Channel closed without Finalize. `writer` drops here; its Drop impl
    // does a best-effort flush (no fsync, no end record). The on-disk file is
    // a valid truncated session that the reader will surface as
    // ReadOutcome::Truncated.
    drop(writer);
}
