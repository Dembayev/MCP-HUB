import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AppInfo,
  InstallServerRequest,
  McpServer,
  ProxyConfig,
} from "@/types/mcp";
import type { LogEntry, ServerStatusChanged } from "@/types/logs";
import type { AgentAction } from "@/types/actions";
import type { PersistedPermission } from "@/types/permissions";

/**
 * Typed wrapper around Tauri's `invoke`. Centralizing this gives us a single
 * place to add logging, error normalization, and mocking for browser-only
 * development (e.g. when running `npm run dev` without `tauri dev`).
 */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    // Tauri errors come through as strings from our AppError Serialize impl.
    const message = typeof err === "string" ? err : String(err);
    throw new Error(message);
  }
}

export const api = {
  listServers: () => call<McpServer[]>("list_servers"),
  getServer: (id: string) => call<McpServer>("get_server", { id }),
  installServer: (request: InstallServerRequest) =>
    call<McpServer>("install_server", { request }),
  startServer: (id: string) => call<McpServer>("start_server", { id }),
  stopServer: (id: string) => call<McpServer>("stop_server", { id }),
  removeServer: (id: string) => call<void>("remove_server", { id }),
  getServerLogs: (id: string, limit?: number) =>
    call<LogEntry[]>("get_server_logs", { id, limit }),
  clearServerLogs: (id: string) => call<void>("clear_server_logs", { id }),
  getServerActions: (id: string, limit?: number) =>
    call<AgentAction[]>("get_server_actions", { id, limit }),
  clearServerActions: (id: string) =>
    call<void>("clear_server_actions", { id }),
  listServerPermissions: (serverId: string) =>
    call<PersistedPermission[]>("list_server_permissions", { serverId }),
  grantPermission: (id: number) => call<void>("grant_permission", { id }),
  revokePermission: (id: number) => call<void>("revoke_permission", { id }),
  appInfo: () => call<AppInfo>("app_info"),
  getProxyConfig: (serverId: string) =>
    call<ProxyConfig>("get_proxy_config", { serverId }),
};

/** Subscribe to live `server-log` events. Returns an unlisten function. */
export function subscribeServerLogs(
  cb: (entry: LogEntry) => void,
): Promise<UnlistenFn> {
  return listen<LogEntry>("server-log", (event) => cb(event.payload));
}

/** Subscribe to `server-status-changed` events. Returns an unlisten function. */
export function subscribeServerStatus(
  cb: (change: ServerStatusChanged) => void,
): Promise<UnlistenFn> {
  return listen<ServerStatusChanged>("server-status-changed", (event) =>
    cb(event.payload),
  );
}

/** Subscribe to classified agent actions. Returns an unlisten function. */
export function subscribeAgentActions(
  cb: (action: AgentAction) => void,
): Promise<UnlistenFn> {
  return listen<AgentAction>("agent-action", (event) => cb(event.payload));
}

/** True when running inside a Tauri window (vs. plain `vite dev` in a browser). */
export const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
