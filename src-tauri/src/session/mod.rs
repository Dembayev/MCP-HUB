//! Session tracing layer — see `docs/SESSION_SCHEMA.md` (frozen at v0.1.0).
//!
//! Architecture (per the spec's "NDJSON-first" rule):
//!
//! - `types`  — the on-the-wire schema, matching §4–§6 and §11 of the spec.
//! - `hash`   — canonical-JSON SHA-256 used for `payload_hash` (§5.5).
//! - `writer` — append-only NDJSON writer with batched fsync. Primary
//!              storage layer. JSON export is a derived fold over this.
//! - `reader` — line-by-line NDJSON parser that gracefully accepts truncated
//!              files (sessions where the process crashed before writing the
//!              `end` record).
//!
//! Anything that wants to record agent activity goes through `SessionWriter`.
//! Anything that wants to inspect a past session goes through `read_ndjson` /
//! `read_json`. Nothing else should touch the on-disk format.

pub mod hash;
pub mod reader;
pub mod types;
pub mod writer;

pub use reader::{read_json, read_ndjson, read_ndjson_from, ReadOutcome};
pub use types::{
    Action, ActionError, Actor, AppInfo, ClientInfo, Kind, NdjsonRecord, Outcome, Redactions,
    SandboxConfig, SandboxDecision, ServerInfo, SessionFile, SessionMeta, Stats, SCHEMA_VERSION,
};
pub use writer::SessionWriter;
