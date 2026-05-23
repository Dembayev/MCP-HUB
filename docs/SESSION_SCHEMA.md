# Session Schema (`session.json` / `session.ndjson`)

**Status:** Draft v0.1 — stable target for MCP Hub v0.1 public alpha
**Owner:** MCP Hub core
**Last updated:** 2026-05-23

A session artifact is the canonical record of everything a single MCP server did during one connected run: every tool call, resource read, prompt fetch, sandbox decision, and lifecycle event, ordered and causally linked so that the trace can be replayed deterministically by a future MCP Hub instance.

This document is the **single source of truth** for the file format. Both Export and Replay must be implementable directly from this spec without further design conversation.

---

## 1. Design principles

1. **Trace, not log.** Every action carries a causal link (`parent_id`) and a monotonic timestamp. Replay reconstructs the cause chain; without it, playback is animation, not debugging.
2. **Two formats, one schema.** `session.json` is a single self-contained artifact. `session.ndjson` is the same data as a stream of newline-delimited records. They are convertible without loss.
3. **Stable on the wire, evolvable in code.** Additive changes never break old readers. Breaking changes bump the major version. Consumers preserve unknown fields on roundtrip.
4. **Cheap trust primitives, no marketing.** Every action has a `payload_hash` so a trace can be verified after-the-fact. We do not call this "tamper-evident" anywhere user-facing.
5. **Replay-grade timing.** Monotonic nanosecond timestamps anchor playback; wall-clock is for humans only.
6. **No payload bloat by default.** Large payloads are truncated with `payload_truncated: true` and the original size is recorded. Full content is opt-in.

---

## 2. File formats

### 2.1 `session.json` (single artifact)

```json
{
  "schema_version": "0.1.0",
  "session": { ... },
  "actions": [ { ... }, { ... } ],
  "stats": { ... }
}
```

One file. Suitable for sharing, archiving, attaching to bug reports.

### 2.2 `session.ndjson` (stream)

Each line is one JSON object with a discriminator `type`:

```
{"type":"meta","schema_version":"0.1.0","session":{...}}
{"type":"action","action":{...}}
{"type":"action","action":{...}}
{"type":"end","ended_at":"...","stats":{...}}
```

Rules:
- The first non-empty line MUST be `type: "meta"`.
- `type: "action"` lines may appear in any quantity, ordered by `seq` ascending.
- The final line (on clean shutdown) MUST be `type: "end"`. An NDJSON file without an `end` record is a **truncated** session — readers must accept it and recompute stats locally.
- Lines beginning with `#` are reserved for human-readable comments and MUST be ignored by parsers.

### 2.3 Conversion

`ndjson → json` is mechanical: collect all `action` records into `actions[]`, merge `meta.session` into `session`, take `stats` from `end` (or recompute).

`json → ndjson` is mechanical: emit `meta`, then one `action` per element of `actions[]`, then `end`.

---

## 3. Top-level schema (`session.json`)

| Field | Type | Required | Notes |
|---|---|---|---|
| `schema_version` | string (semver) | yes | Current: `"0.1.0"` |
| `session` | object | yes | See §4 |
| `actions` | array<Action> | yes | Ordered by `seq` ascending |
| `stats` | object | no | Computed at export time; readers MAY recompute |

---

## 4. `session` metadata

```json
{
  "id": "01HXYZ...",                // ULID — sortable, time-prefixed
  "started_at": "2026-05-23T14:02:11.137Z",
  "ended_at":   "2026-05-23T14:08:42.901Z",
  "started_mono_ns": 0,             // always 0 — anchor for monotonic timestamps
  "app": {
    "name": "mcp-hub",
    "version": "0.1.0",
    "build": "a1b2c3d",
    "os": "macos-14.4-arm64"
  },
  "server": {
    "id": "filesystem",
    "name": "Filesystem",
    "version": "0.6.2",
    "transport": "stdio",           // "stdio" | "sse" | "ws"
    "command": ["npx","@modelcontextprotocol/server-filesystem","/Users/x/Projects"],
    "capabilities": ["tools","resources","prompts"]
  },
  "client": {
    "name": "claude-desktop",       // "claude-desktop" | "cursor" | "windsurf" | "cline" | "unknown"
    "version": "0.9.4"
  },
  "sandbox": {
    "mode": "enforce",              // "off" | "observe" | "enforce"
    "fs_allow":  ["/Users/x/Projects"],
    "fs_deny":   ["~/.ssh", "~/.aws"],
    "net_allow": ["api.openai.com"],
    "net_default": "deny"
  },
  "redactions": {                   // optional, present only if any redaction was applied
    "paths": ["args.path"],         // dotted JSON-pointers that were stripped
    "policy": "user-export"         // free-form policy id
  }
}
```

Notes:
- `id` is a [ULID](https://github.com/ulid/spec). Sortable by creation time, 26 chars, URL-safe. Preferred over UUIDv4 for trace IDs throughout.
- `started_mono_ns` is always `0` and exists only to make the file self-documenting. All action `ts_mono_ns` values are offsets from this anchor.
- `redactions` is **absent** when no redactions occurred. Its presence means the trace is sanitized — replay against it will not produce identical side effects.

---

## 5. `Action` record

The atomic unit of a trace. One per MCP-protocol event or sandbox decision.

```json
{
  "id":         "01HXYZ...",        // ULID
  "seq":        42,                 // monotonic per session, gap-free, starts at 0
  "parent_id":  "01HXYZ...",        // ULID of the action that caused this one, or null
  "cause_id":   "01HXYZ...",        // ULID of the root trigger (e.g. model turn), or null
  "ts_wall":    "2026-05-23T14:02:13.412Z",
  "ts_mono_ns": 2275000000,         // offset from session.started_mono_ns
  "duration_ns": 18324000,          // null until action completes

  "kind":   "tool_call",            // see §5.1
  "actor":  "agent",                // "agent" | "user" | "system" | "sandbox"
  "tool":   "read_file",            // method/tool name, null for non-tool kinds
  "args":   { "path": "package.json" },
  "result": { "content": "...", "mime": "application/json" },
  "outcome":"ok",                   // "ok" | "error" | "denied" | "timeout" | "cancelled"
  "error":  null,                   // object when outcome != "ok", see §5.3
  "decision": null,                 // object when sandbox intervened, see §5.4

  "payload_hash": "sha256:9f86d0...",
  "payload_truncated": false,
  "payload_size_bytes": 2148,

  "tags": ["fs","read"]
}
```

### 5.1 `kind`

| Value | Meaning |
|---|---|
| `tool_call` | MCP `tools/call` request and its response |
| `resource_read` | MCP `resources/read` |
| `resource_list` | MCP `resources/list` |
| `prompt_get` | MCP `prompts/get` |
| `completion` | MCP `completion/complete` |
| `notification` | Unsolicited server-to-client message |
| `sandbox_decision` | Standalone sandbox event not tied to one request (e.g. policy reload) |
| `session_event` | `start`, `end`, `disconnect`, `error` |

Readers MUST accept unknown `kind` values and surface them as `"unknown"` rather than rejecting the file.

### 5.2 Causal linking

- `parent_id` is the immediate cause: e.g. a `sandbox_decision` action's parent is the `tool_call` it intercepted.
- `cause_id` is the root trigger of a chain: typically the model-turn ID or user-message ID that ultimately initiated everything downstream. Multiple actions can share a `cause_id`.

Both fields are nullable. A top-level event (e.g. `session_event: start`) has `parent_id: null` and `cause_id: null`.

### 5.3 `error`

```json
{
  "code": "ENOENT",
  "message": "File not found: /etc/shadow",
  "source": "tool" | "transport" | "sandbox" | "internal",
  "data": { ... }
}
```

### 5.4 `decision` (sandbox)

Present when the sandbox produced a verdict, whether allow or deny:

```json
{
  "verdict":   "deny",              // "allow" | "deny" | "prompt"
  "rule_id":   "fs.deny.ssh",
  "reason":    "Path matches fs_deny pattern ~/.ssh",
  "mode":      "enforce",           // sandbox mode at decision time
  "prompted":  false,               // true if user was asked at runtime
  "prompt_resolution": null         // "allow_once" | "allow_session" | "deny" | null
}
```

A denied `tool_call` produces TWO actions:
1. The `tool_call` itself with `outcome: "denied"` and `decision` set.
2. Optionally a separate `sandbox_decision` action whose `parent_id` points at the `tool_call`, used when the sandbox emits richer telemetry (e.g. policy trace).

For v0.1, prefer the single-action form (`tool_call` with embedded `decision`). Separate `sandbox_decision` actions are reserved for future use.

### 5.5 `payload_hash`

- Hash is computed over the **canonicalized concatenation** of `args` and `result`:
  ```
  sha256( canonical_json(args) || 0x1f || canonical_json(result) )
  ```
  where `0x1f` is the ASCII unit separator and `canonical_json` is [RFC 8785 JCS](https://www.rfc-editor.org/rfc/rfc8785).
- When `result` is null (action incomplete), only `args` is hashed and the prefix is `sha256-partial:`.
- Format: `sha256:<hex>` (64 hex chars) or `sha256-partial:<hex>`.

### 5.6 Truncation

When `args` or `result` exceeds the configured cap (default: 64 KiB per side):
- The value in the record is replaced with `{"$truncated": true, "preview": "<first 256 chars>"}`.
- `payload_truncated: true`.
- `payload_size_bytes` reflects the **original** combined size.
- `payload_hash` is still computed over the **original** untruncated content.

This means hashes match across truncated and full-fidelity exports of the same session — important for replay verification.

---

## 6. `stats`

Computed at export. Readers MAY recompute and MUST tolerate disagreement (treat author's stats as advisory).

```json
{
  "total_actions": 47,
  "by_outcome": { "ok": 41, "denied": 3, "error": 2, "timeout": 1 },
  "by_kind":    { "tool_call": 38, "resource_read": 7, "session_event": 2 },
  "denied_count": 3,
  "error_count":  2,
  "duration_ms":  391764,
  "avg_action_ms": 12.4,
  "p95_action_ms": 78.0,
  "bytes_in":  142309,
  "bytes_out": 89124
}
```

---

## 7. Forward compatibility

**Versioning.** `schema_version` follows semver:
- **Patch** (`0.1.0 → 0.1.1`): documentation, clarifications. Bytes unchanged.
- **Minor** (`0.1.0 → 0.2.0`): additive only. New fields, new enum variants, new `kind` values. Old readers MUST tolerate by ignoring unknown fields and surfacing unknown enums as `"unknown"`.
- **Major** (`0.x → 1.0`): may rename or remove. Readers refuse to parse a major version higher than they understand.

**Reader rules.**
1. Preserve unknown fields on roundtrip.
2. Treat unknown enum values as the string `"unknown"` for filtering; preserve original on export.
3. Never fail the parse on a missing optional field; treat as `null`.
4. Refuse to parse `schema_version` with a higher major than the reader supports. Emit a clear error.

**Writer rules.**
1. Emit every required field.
2. Sort keys lexicographically inside `canonical_json` for hashing; the export file itself MAY use any key order.
3. Use `null` for absent optional values, not omission, when the field semantically exists (e.g. `parent_id: null` on a root action). Use omission only for truly absent structures (e.g. `redactions`).

---

## 8. Replay determinism

A replay engine consumes a session and reconstructs the trace UI as if the events were happening live. Determinism requires:

1. **Ordering** — replay walks actions sorted by `seq`. `ts_mono_ns` is ordering-consistent with `seq` (monotonic non-decreasing).
2. **Timing** — between two consecutive actions, the replay engine MAY:
   - Use real wall-clock spacing (`Δ = ts_mono_ns[i+1] - ts_mono_ns[i]`) — "real-time replay"
   - Compress to a fixed step — "scrubber/step-through"
   - Skip ahead on user scrub — "seek"
   - In all modes, no action is shown before its parent has been shown.
3. **State reconstruction** — replay rebuilds a live view from action records alone; the engine must not depend on side-effects of the original run (e.g. it does not actually re-read files).
4. **Causal display** — when an action is selected, the UI must be able to highlight its `parent_id` and all descendants. This is the feature that makes replay debugging, not playback.

For v0.1, replay is local-only and supports: deterministic playback, scrubber, step-through. Video export is **explicitly out of scope** for v0.1.

---

## 9. Privacy & redaction

Three levels, applied at export time:

| Level | Behavior |
|---|---|
| `none` (default) | Full fidelity. Hashes match original. |
| `paths` | Strip values at named JSON pointers (e.g. `args.path`, `result.content`). Listed in `session.redactions.paths`. Hash recomputed post-redaction; mark with `payload_hash_redacted: true`. |
| `anonymize` | Replace identifying strings (paths, hostnames, usernames) with stable pseudonyms (`<path:1>`, `<host:2>`). Mapping is NOT included in the export. |

Redaction is one-way. Replay against a redacted trace works for UI, but cannot verify against original hashes.

---

## 10. Example: a 3-action session

```json
{
  "schema_version": "0.1.0",
  "session": {
    "id": "01HXP4M9C0KTQ8N5W2Y3Z6R7AB",
    "started_at": "2026-05-23T14:02:11.137Z",
    "ended_at":   "2026-05-23T14:02:14.892Z",
    "started_mono_ns": 0,
    "app":    { "name":"mcp-hub","version":"0.1.0","build":"a1b2c3d","os":"macos-14.4-arm64" },
    "server": { "id":"filesystem","name":"Filesystem","version":"0.6.2","transport":"stdio","command":["npx","@modelcontextprotocol/server-filesystem","/Users/x/Projects"],"capabilities":["tools","resources"] },
    "client": { "name":"claude-desktop","version":"0.9.4" },
    "sandbox":{ "mode":"enforce","fs_allow":["/Users/x/Projects"],"fs_deny":["~/.ssh"],"net_allow":[],"net_default":"deny" }
  },
  "actions": [
    {
      "id":"01HXP4M9C1AAAAAAAAAAAAAAAA","seq":0,"parent_id":null,"cause_id":null,
      "ts_wall":"2026-05-23T14:02:11.137Z","ts_mono_ns":0,"duration_ns":null,
      "kind":"session_event","actor":"system","tool":null,
      "args":{"event":"start"},"result":null,"outcome":"ok","error":null,"decision":null,
      "payload_hash":"sha256:3a7bd3e2360a3d29eea436fcfb7e44c735d117c42d1c1835420b6b9942dd4f1b",
      "payload_truncated":false,"payload_size_bytes":17,"tags":["lifecycle"]
    },
    {
      "id":"01HXP4M9C2BBBBBBBBBBBBBBBB","seq":1,"parent_id":null,"cause_id":"01HXP4M9C1AAAAAAAAAAAAAAAA",
      "ts_wall":"2026-05-23T14:02:13.412Z","ts_mono_ns":2275000000,"duration_ns":18324000,
      "kind":"tool_call","actor":"agent","tool":"read_file",
      "args":{"path":"/Users/x/Projects/site/package.json"},
      "result":{"content":"{\"name\":\"site\"...}","mime":"application/json"},
      "outcome":"ok","error":null,"decision":{"verdict":"allow","rule_id":"fs.allow.projects","reason":"Path inside fs_allow","mode":"enforce","prompted":false,"prompt_resolution":null},
      "payload_hash":"sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
      "payload_truncated":false,"payload_size_bytes":2148,"tags":["fs","read"]
    },
    {
      "id":"01HXP4M9C3CCCCCCCCCCCCCCCC","seq":2,"parent_id":null,"cause_id":"01HXP4M9C1AAAAAAAAAAAAAAAA",
      "ts_wall":"2026-05-23T14:02:14.108Z","ts_mono_ns":2971000000,"duration_ns":4102000,
      "kind":"tool_call","actor":"agent","tool":"write_file",
      "args":{"path":"/Users/x/.ssh/config","content":"Host evil ..."},
      "result":null,"outcome":"denied","error":{"code":"SANDBOX_DENY","message":"Write to ~/.ssh/config blocked by policy","source":"sandbox","data":null},
      "decision":{"verdict":"deny","rule_id":"fs.deny.ssh","reason":"Path matches fs_deny pattern ~/.ssh","mode":"enforce","prompted":false,"prompt_resolution":null},
      "payload_hash":"sha256-partial:7d865e959b2466918c9863afca942d0fb89d7c9ac0c99bafc3749504ded97730",
      "payload_truncated":false,"payload_size_bytes":78,"tags":["fs","write","denied"]
    }
  ],
  "stats": {
    "total_actions": 3,
    "by_outcome": { "ok": 2, "denied": 1 },
    "by_kind":    { "session_event": 1, "tool_call": 2 },
    "denied_count": 1,
    "error_count":  0,
    "duration_ms": 3755,
    "avg_action_ms": 7.5,
    "p95_action_ms": 18.3,
    "bytes_in": 142,
    "bytes_out": 2148
  }
}
```

This three-action session is the canonical demo trace: lifecycle start, an allowed read, and the denial moment. It is exactly the shape of file the export button must produce on day one.

---

## 11. Reference Rust types

These compile-on-sight types are the source of truth on the Rust side. UI/replay can derive TypeScript types from these via `ts-rs` or hand-mirror.

```rust
#[derive(Serialize, Deserialize)]
pub struct SessionFile {
    pub schema_version: String,    // "0.1.0"
    pub session: SessionMeta,
    pub actions: Vec<Action>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<Stats>,
}

#[derive(Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: Ulid,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub started_mono_ns: u64,          // always 0
    pub app: AppInfo,
    pub server: ServerInfo,
    pub client: ClientInfo,
    pub sandbox: SandboxConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redactions: Option<Redactions>,
}

#[derive(Serialize, Deserialize)]
pub struct Action {
    pub id: Ulid,
    pub seq: u64,
    pub parent_id: Option<Ulid>,
    pub cause_id: Option<Ulid>,
    pub ts_wall: DateTime<Utc>,
    pub ts_mono_ns: u64,
    pub duration_ns: Option<u64>,
    pub kind: Kind,
    pub actor: Actor,
    pub tool: Option<String>,
    pub args: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub outcome: Outcome,
    pub error: Option<ActionError>,
    pub decision: Option<SandboxDecision>,
    pub payload_hash: String,          // "sha256:..." or "sha256-partial:..."
    pub payload_truncated: bool,
    pub payload_size_bytes: u64,
    pub tags: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    ToolCall, ResourceRead, ResourceList, PromptGet,
    Completion, Notification, SandboxDecision, SessionEvent,
    #[serde(other)] Unknown,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Actor { Agent, User, System, Sandbox, #[serde(other)] Unknown }

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome { Ok, Error, Denied, Timeout, Cancelled, #[serde(other)] Unknown }
```

`#[serde(other)]` on each enum is what implements the "unknown variants survive" rule from §7.

---

## 12. Open questions (deferred to v0.2)

These are explicit non-decisions for v0.1. Listed here so they don't get re-litigated mid-implementation:

1. **Streaming tool responses.** Some MCP tools may stream. v0.1 collapses streamed results into a single `result` blob with `streamed: true` in `tags`. Multi-chunk records are deferred.
2. **Cross-session references.** No way to link two sessions yet (e.g. "this trace continues from session X"). When needed, add `session.continues_from: <ulid>`.
3. **Live tail protocol.** `mcp-hub tail` is in scope, but its wire format (probably NDJSON-over-WS) is a separate spec.
4. **Signed exports.** A signing layer on top of `payload_hash` (e.g. session-level signature) is deliberately out of scope. We are not building an audit system; we are building a debugger.
5. **Compression.** `.json.zst` for large traces. Trivial to add — not required for v0.1.

---

## 13. Implementation checklist for v0.1

For Export to ship, the implementation must:

- [ ] Emit `schema_version: "0.1.0"`.
- [ ] Capture every MCP request/response as an `Action` with `seq`, `ts_mono_ns`, `parent_id` (where applicable), `cause_id` (model-turn correlation).
- [ ] Compute `payload_hash` over canonical-JSON of args+result.
- [ ] Apply truncation cap (default 64 KiB) and set `payload_truncated`, `payload_size_bytes`.
- [ ] On sandbox decisions, populate `outcome` and `decision` on the originating `tool_call`.
- [ ] Write `session.json` on one-click export.
- [ ] Write `session.ndjson` to disk continuously during the session (append-only, fsync per record group), so a crash leaves a valid truncated trace.
- [ ] Provide a reader that accepts both formats and accepts truncated NDJSON gracefully.

Replay (v0.1 alpha) needs the reader plus a scrubber/step-through UI. Nothing in the schema needs to change to support replay — that's the whole point of fixing the spec first.
