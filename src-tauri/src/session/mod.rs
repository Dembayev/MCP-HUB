//! Session tracing layer — see `docs/SESSION_SCHEMA.md` (frozen at v0.1.0).
//!
//! Architecture (per the spec's "NDJSON-first" rule):
//!
//! - `types`   — the on-the-wire schema, matching §4–§6 and §11 of the spec.
//! - `hash`    — canonical-JSON SHA-256 used for `payload_hash` (§5.5).
//! - `writer`  — sync append-only NDJSON writer with batched fsync. Internal
//!               building block — production code uses [`SessionHandle`].
//! - `reader`  — line-by-line NDJSON parser that gracefully accepts truncated
//!               files and sorts by `seq` per spec §3.
//! - `partial` — immutable `PartialAction` + completion folds (request →
//!               response/deny → final `Action`).
//! - `runtime` — async mpsc-task pipeline. [`SessionHandle`] owns lifecycle,
//!               [`SessionAppender`] is the clonable producer-side handle.
//!
//! ## Production usage
//!
//! Anything that wants to record agent activity goes through
//! [`SessionAppender::append`]. The proxy and any other producer never touch
//! a file directly. Anything that wants to inspect a past session goes
//! through [`read_ndjson`] / [`read_json`].

pub mod demo;
pub mod hash;
pub mod partial;
pub mod reader;
pub mod runtime;
pub mod types;
pub mod writer;

pub use partial::PartialAction;
pub use reader::{read_json, read_ndjson, read_ndjson_from, ReadOutcome};
pub use runtime::{SessionAppender, SessionHandle, DEFAULT_CHANNEL_CAPACITY};
pub use types::{
    Action, ActionError, Actor, AppInfo, ClientInfo, Kind, NdjsonRecord, Outcome, Redactions,
    SandboxConfig, SandboxDecision, ServerInfo, SessionFile, SessionMeta, Stats, SCHEMA_VERSION,
};
pub use writer::SessionWriter;
