/**
 * Type definitions mirroring the Rust IPC payloads.
 * Keep these in lockstep with `src-tauri/src/db/models.rs`.
 */

export type Transport = "stdio" | "sse" | "http";
export type ServerStatus = "stopped" | "starting" | "running" | "crashed";

export interface McpServer {
  id: string;
  name: string;
  description: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  transport: Transport;
  status: ServerStatus;
  installedAt: string; // ISO 8601
  updatedAt: string; // ISO 8601
  version: string | null;
  source: string | null;
  iconUrl: string | null;
}

export interface InstallPermissionRequest {
  scope: string;
  target?: string | null;
  reason?: string | null;
}

export interface InstallServerRequest {
  name: string;
  description?: string;
  command: string;
  args?: string[];
  env?: Record<string, string>;
  transport?: Transport;
  version?: string | null;
  source?: string | null;
  iconUrl?: string | null;
  /** Permissions the user consented to in the install dialog. */
  permissions?: InstallPermissionRequest[];
}

export interface AppInfo {
  version: string;
  dataDir: string;
  /** "macos-sandbox-exec" on macOS, "noop" elsewhere. */
  sandboxEnforcement: "macos-sandbox-exec" | "noop" | string;
}

/** Returned by `get_proxy_config` — everything the UI needs to render the
 * "Use in Claude Desktop" snippet. */
export interface ProxyConfig {
  serverId: string;
  serverName: string;
  proxyPath: string;
  /** Pre-formatted JSON snippet ready to paste into the client config. */
  snippet: string;
  sockPath: string;
}
