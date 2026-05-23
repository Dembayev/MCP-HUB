/**
 * Semantic agent actions surfaced on the Timeline page. Mirrors
 * `src-tauri/src/mcp/agent.rs`.
 */

export type ActionKind =
  | "fs-read"
  | "fs-write"
  | "browser-open"
  | "http-fetch"
  | "terminal-exec"
  | "memory-store"
  | "search"
  | "tool-call"
  | "other";

export type ActionStatus =
  | "pending"
  | "success"
  | "denied"
  | "failed"
  | "cancelled";

export interface AgentAction {
  /** Stable id used as a React key AND for lifecycle updates. */
  id: string;
  serverId: string;
  kind: ActionKind;
  toolName: string;
  /** Path, URL, or command — the headline of the card. */
  target: string | null;
  /** Full JSON-RPC `params.arguments`, for the expanded card view. */
  params: Record<string, unknown> | null;
  /** Original JSON-RPC request id, if any. */
  requestId: number | string | null;
  status: ActionStatus;
  /** When `status === "denied"`, the reason surfaced to the user. */
  deniedReason: string | null;
  /** When `status === "failed"`, the server-reported error. */
  error: string | null;
  /** Round-trip latency, set when the action reaches a terminal state. */
  latencyMs: number | null;
  timestamp: string;
}
