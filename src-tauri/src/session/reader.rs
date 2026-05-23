//! Session readers — NDJSON (streaming, truncation-tolerant) and JSON (the
//! exported snapshot form).
//!
//! ## Truncation handling
//!
//! A truncated NDJSON file is a session whose process died before
//! [`SessionWriter::finalize`](super::writer::SessionWriter::finalize) ran.
//! In practice this means one of:
//!
//! - The `end` record is missing entirely.
//! - The final line is a partial action that wasn't fully written before the
//!   crash (no trailing newline; serde_json fails to parse it).
//!
//! Both cases are NORMAL. We surface them via [`ReadOutcome::Truncated`] so
//! callers can show a "session ended unexpectedly" badge in the UI, but we
//! never refuse to read the file: a truncated trace is still a useful trace.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::error::{AppError, AppResult};

use super::types::{Action, NdjsonRecord, SessionFile, SessionMeta, Stats};

/// Result of reading an NDJSON session file.
#[derive(Debug, Clone)]
pub enum ReadOutcome {
    /// File contained `meta` … `action`* … `end`. Stats came from the writer.
    Complete(SessionFile),
    /// File was missing the `end` record (process likely crashed). Stats are
    /// recomputed on the fly. The contained `SessionFile` has
    /// `session.ended_at = None`.
    Truncated(SessionFile),
}

impl ReadOutcome {
    /// Convenience: get the [`SessionFile`] regardless of completeness.
    pub fn into_file(self) -> SessionFile {
        match self {
            ReadOutcome::Complete(f) | ReadOutcome::Truncated(f) => f,
        }
    }

    pub fn is_truncated(&self) -> bool {
        matches!(self, ReadOutcome::Truncated(_))
    }
}

/// Read an NDJSON session file from disk.
///
/// See [`ReadOutcome`] for truncation handling. A missing or empty file is
/// an error; a file with only a `meta` record is a valid (very short)
/// truncated session.
pub fn read_ndjson(path: impl AsRef<Path>) -> AppResult<ReadOutcome> {
    let file = File::open(path.as_ref())?;
    read_ndjson_from(BufReader::new(file))
}

/// Parse NDJSON from any `BufRead` source. Useful for tests against
/// in-memory bytes without touching the filesystem.
pub fn read_ndjson_from(reader: impl BufRead) -> AppResult<ReadOutcome> {
    let mut meta: Option<(String, SessionMeta)> = None;
    let mut actions: Vec<Action> = Vec::new();
    let mut end: Option<(chrono::DateTime<chrono::Utc>, Stats)> = None;

    for (lineno, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(l) => l,
            // If a partial trailing line caused the read to fail, treat as
            // truncation rather than refusing the whole file.
            Err(_) => break,
        };
        let line = line.trim();

        // §2.2: lines starting with `#` are reserved comments and ignored;
        // blank lines are also tolerated.
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // A line that fails to parse as JSON is almost certainly a partial
        // last line from a crash mid-write. Stop reading; everything before
        // is good.
        let rec: NdjsonRecord = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(_) => break,
        };

        match rec {
            NdjsonRecord::Meta {
                schema_version,
                session,
            } => {
                if meta.is_some() {
                    return Err(AppError::Internal(format!(
                        "session reader: duplicate meta record at line {}",
                        lineno + 1
                    )));
                }
                meta = Some((schema_version, session));
            }
            NdjsonRecord::Action { action } => {
                if meta.is_none() {
                    return Err(AppError::Internal(
                        "session reader: action record before meta".to_string(),
                    ));
                }
                actions.push(action);
            }
            NdjsonRecord::End { ended_at, stats } => {
                end = Some((ended_at, stats));
                // §2.2: end MUST be the final record. We don't read past it
                // (any post-end records are a bug, not a feature).
                break;
            }
        }
    }

    let (schema_version, mut session) = meta.ok_or_else(|| {
        AppError::Internal("session reader: file has no meta record".to_string())
    })?;

    // Spec §3: `actions[]` is ordered by `seq` ascending. On-disk NDJSON line
    // order is NOT guaranteed semantic — a writer-task may receive completions
    // out of arrival order under concurrent in-flight requests. The reader is
    // the canonical sorting point so all downstream consumers (Timeline, JSON
    // export, replay) get a stable ordered view.
    actions.sort_by_key(|a| a.seq);

    match end {
        Some((ended_at, stats)) => {
            session.ended_at = Some(ended_at);
            Ok(ReadOutcome::Complete(SessionFile {
                schema_version,
                session,
                actions,
                stats: Some(stats),
            }))
        }
        None => {
            // Truncated: recompute stats so the UI has something to show.
            session.ended_at = None;
            let stats = recompute_stats(&actions);
            Ok(ReadOutcome::Truncated(SessionFile {
                schema_version,
                session,
                actions,
                stats: Some(stats),
            }))
        }
    }
}

/// Read a single-artifact `session.json` file.
///
/// The JSON form is always complete — if the writer crashed mid-export, the
/// file simply doesn't exist (we write to a temp path and rename atomically
/// — see the export path in §13). So unlike NDJSON we don't need a truncation
/// variant here.
pub fn read_json(path: impl AsRef<Path>) -> AppResult<SessionFile> {
    let bytes = std::fs::read(path.as_ref())?;
    let file: SessionFile = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::Internal(format!("session.json parse: {e}")))?;
    Ok(file)
}

/// Recompute stats from the action stream — used both for truncated sessions
/// and as a reader-side sanity check against writer-emitted stats.
fn recompute_stats(actions: &[Action]) -> Stats {
    use std::collections::BTreeMap;

    let mut by_outcome: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_kind: BTreeMap<String, u64> = BTreeMap::new();
    let mut denied = 0u64;
    let mut errors = 0u64;
    let mut durations_ms: Vec<f64> = Vec::with_capacity(actions.len());

    for a in actions {
        // serde_plain-style: serialize the enum back to its wire token for the
        // string-keyed counter maps. Falling back to "unknown" matches the
        // forward-compat rule (unknown variants survive but bucket together).
        let outcome_key = serde_json::to_value(a.outcome)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".to_string());
        let kind_key = serde_json::to_value(a.kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".to_string());

        *by_outcome.entry(outcome_key.clone()).or_default() += 1;
        *by_kind.entry(kind_key).or_default() += 1;

        if outcome_key == "denied" {
            denied += 1;
        }
        if outcome_key == "error" {
            errors += 1;
        }
        if let Some(dur) = a.duration_ns {
            durations_ms.push(dur as f64 / 1_000_000.0);
        }
    }

    let duration_ms = actions
        .iter()
        .map(|a| a.ts_mono_ns + a.duration_ns.unwrap_or(0))
        .max()
        .unwrap_or(0)
        / 1_000_000;

    let avg_action_ms = if durations_ms.is_empty() {
        0.0
    } else {
        durations_ms.iter().sum::<f64>() / durations_ms.len() as f64
    };

    let p95_action_ms = percentile(&mut durations_ms, 0.95);

    Stats {
        total_actions: actions.len() as u64,
        by_outcome,
        by_kind,
        denied_count: denied,
        error_count: errors,
        duration_ms,
        avg_action_ms,
        p95_action_ms,
        bytes_in: 0,  // not tracked at this layer in v0.1
        bytes_out: 0, // ditto
    }
}

fn percentile(samples: &mut [f64], p: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = ((samples.len() - 1) as f64 * p).round() as usize;
    samples[rank]
}
