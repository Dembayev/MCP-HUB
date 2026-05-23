import type { ServerStatus } from "./mcp";

export type LogStream = "stdout" | "stderr";

export interface LogEntry {
  serverId: string;
  stream: LogStream;
  message: string;
  /** ISO 8601 timestamp from the Rust side. */
  timestamp: string;
}

/** Payload of the `server-status-changed` Tauri event. */
export interface ServerStatusChanged {
  id: string;
  status: ServerStatus;
}
