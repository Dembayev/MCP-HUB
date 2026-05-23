/**
 * Session trace types — mirror `src-tauri/src/session/types.rs` and
 * `src-tauri/src/commands/sessions.rs`.
 *
 * Note: the schema types (Action, SessionFile, etc.) use **snake_case**
 * field names because the wire format is the same as the on-disk NDJSON
 * format (frozen at v0.1.0 in `docs/SESSION_SCHEMA.md`). SessionSummary
 * is a UI-facing DTO and uses camelCase per the rest of the frontend.
 */

// ---------------------------------------------------------------------------
// Enums — mirror Rust enums with #[serde(rename_all = "snake_case")]
// ---------------------------------------------------------------------------

export type ActionKind =
  | "tool_call"
  | "resource_read"
  | "resource_list"
  | "prompt_get"
  | "completion"
  | "notification"
  | "sandbox_decision"
  | "session_event"
  | "unknown";

export type ActionActor = "agent" | "user" | "system" | "sandbox" | "unknown";

export type ActionOutcome =
  | "ok"
  | "error"
  | "denied"
  | "timeout"
  | "cancelled"
  | "unknown";

// ---------------------------------------------------------------------------
// On-disk schema types (snake_case to match spec)
// ---------------------------------------------------------------------------

export interface AppInfo {
  name: string;
  version: string;
  build: string;
  os: string;
}

export interface ServerInfo {
  id: string;
  name: string;
  version: string;
  transport: string;
  command: string[];
  capabilities: string[];
}

export interface ClientInfo {
  name: string;
  version: string;
}

export interface SandboxConfig {
  mode: string;
  fs_allow: string[];
  fs_deny: string[];
  net_allow: string[];
  net_default: string;
}

export interface SandboxDecision {
  verdict: string;
  rule_id: string;
  reason: string;
  mode: string;
  prompted: boolean;
  prompt_resolution: string | null;
}

export interface ActionError {
  code: string;
  message: string;
  source: string;
  data: unknown | null;
}

export interface SessionMeta {
  id: string;
  started_at: string; // RFC 3339
  ended_at: string | null;
  started_mono_ns: number;
  app: AppInfo;
  server: ServerInfo;
  client: ClientInfo;
  sandbox: SandboxConfig;
  redactions?: { paths: string[]; policy: string } | null;
}

export interface Action {
  id: string; // ULID string
  seq: number;
  parent_id: string | null;
  cause_id: string | null;
  ts_wall: string; // RFC 3339
  ts_mono_ns: number;
  duration_ns: number | null;
  kind: ActionKind;
  actor: ActionActor;
  tool: string | null;
  args: unknown | null;
  result: unknown | null;
  outcome: ActionOutcome;
  error: ActionError | null;
  decision: SandboxDecision | null;
  payload_hash: string;
  payload_truncated: boolean;
  payload_size_bytes: number;
  tags: string[];
}

export interface Stats {
  total_actions: number;
  by_outcome: Record<string, number>;
  by_kind: Record<string, number>;
  denied_count: number;
  error_count: number;
  duration_ms: number;
  avg_action_ms: number;
  p95_action_ms: number;
  bytes_in: number;
  bytes_out: number;
}

export interface SessionFile {
  schema_version: string;
  session: SessionMeta;
  actions: Action[];
  stats: Stats | null;
}

// ---------------------------------------------------------------------------
// UI-facing summary (camelCase, from commands/sessions.rs SessionSummary)
// ---------------------------------------------------------------------------

export type SessionStatus = "complete" | "truncated";

export interface SessionSummary {
  id: string;
  serverId: string;
  serverName: string;
  clientName: string;
  startedAt: string;
  endedAt: string | null;
  actionCount: number;
  deniedCount: number;
  errorCount: number;
  durationMs: number;
  status: SessionStatus;
  path: string;
}
