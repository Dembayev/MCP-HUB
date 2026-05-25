# Changelog

All notable changes to MCP Hub are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The session-trace wire format follows its own version (`schema_version` in
NDJSON); see [`docs/SESSION_SCHEMA.md`](docs/SESSION_SCHEMA.md).

## [Unreleased]

Working toward `v0.1.0-alpha`. See [README roadmap](README.md#roadmap) for
in-flight items.

## [0.1.0-alpha] — pre-release

The pre-release that opens the project to the public. Every action your AI
agent takes is captured, replayable, and gated through user approval.

### Added

#### Session trace primitive

- Frozen wire format `schema_version: "0.1.0"` (`docs/SESSION_SCHEMA.md`)
- Append-only NDJSON writer with batched fsync (default batch size 16)
- Truncation-tolerant reader: a session whose process crashed mid-write
  is still readable; stats are recomputed from the action stream
- Reader sorts by `seq` per spec §3, so disk-arrival order is independent
  of logical order
- `mpsc`-based async runtime (`SessionHandle` / `SessionAppender`) —
  no `Mutex` in the hot path; backpressure via channel capacity
- Immutable `PartialAction` with `complete` / `complete_error` /
  `complete_denied` folds
- Canonical-JSON SHA-256 `payload_hash` for tamper-evidence
- Tests: 27 unit + 9 roundtrip + 4 concurrency (including N=1000
  no-loss stress under parallel completion)

#### Timeline tab — persistent trace browser

- Sidebar lists sessions found in `<data_dir>/sessions/*.ndjson`,
  newest-first, with truncation / denial badges
- Trace view: meta header (server, client, sandbox mode, outcome counters,
  status badge) over a dense action stream
- DevTools / Chrome-trace aesthetic: outcome-coded dot + monospace
  `seq`/tool/target/duration/timestamp, inline expansion with full
  args/result/decision/error/hash
- Live refresh via 1 Hz polling; reference-stable so playback isn't
  reset on every tick

#### Replay engine

- `useReplay` hook: position (action index), playing, speed
  (0.5×/1×/2×/5×/10×/instant); deterministic timing via `ts_mono_ns`
  deltas, clamped to keep dense traces from storming and gappy
  traces from dead-airing
- Scrubber strip: one outcome-colored tick per action, draggable
  playhead with pointer-capture seek, keyboard arrow/Home/End support
- Replay controls: play/pause, step ±, speed selector, "Next denial"
  jump affordance, reset
- Current row auto-scrolls into view; amber marker over `decision.prompted`
  actions (visual "user was in the loop")

#### Runtime approval flow

- `Decision::AskUser` variant: when a request hits an ungranted scope,
  the proxy emits an `approval-requested` Tauri event and awaits a
  `oneshot::Sender` registered in the in-memory `ApprovalRegistry`
- Non-dismissable modal: Allow Once / Always Allow / Deny + static
  risk classification (low/medium/high — no heuristic engine)
- "Always Allow" persists the scope to the permissions DB AND
  refreshes the in-memory snapshot so the prompt doesn't re-fire
  in this session
- FIFO queue: bursts are surfaced one at a time
- Decision flows back into the session trace as
  `decision.prompted: true` + `prompt_resolution: "allow_once" |
  "allow_session" | "deny"`

#### Proxy instrumentation

- Every classified JSON-RPC `tools/call` on a proxied MCP server is
  captured as a session-trace Action via `SessionAppender`
- Lifecycle: `session_event: start` action emitted at connection;
  end record + fsync on disconnect
- Causal chain: every downstream action carries the start event's
  ULID as its `cause_id`
- `seq` allocated monotonically per session via `AtomicU64`;
  `ts_mono_ns` anchored at connection start
- On denial (sandbox or user-mediated): the action is folded via
  `complete_denied` and emitted immediately; on response match
  (success or server error): folded via `complete` /
  `complete_error` with the response body

#### Infrastructure

- Tauri 2 + Rust + React + TypeScript + Tailwind + shadcn/ui scaffold
- SQLite-backed permissions, logs, action history
- Per-server platform sandbox profiles (macOS `sandbox-exec`)
- CI: cargo fmt / build / test / clippy on macOS, frontend tsc +
  build on Ubuntu

[Unreleased]: https://github.com/Dembayev/MCP-HUB/compare/v0.1.0-alpha...HEAD
[0.1.0-alpha]: https://github.com/Dembayev/MCP-HUB/releases/tag/v0.1.0-alpha
