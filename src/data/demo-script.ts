import type { AgentAction, ActionKind, ActionStatus } from "@/types/actions";

/**
 * Realistic 10-step agent flow used by Demo Mode on the Timeline page.
 *
 * Each step now has a two-phase lifecycle:
 *   1. A pending card appears at `delay` ms after the previous step's
 *      appearance.
 *   2. The same card settles into `finalStatus` after `durationMs`,
 *      gaining a `latencyMs` badge.
 *
 * This is what makes the timeline feel alive instead of "log dump".
 *
 * Tuning notes:
 *  - Keep the total runtime in the 12-18 second band for a tight clip.
 *  - Mix instant-feeling ops (~250-500 ms latency) with slow ones (HTTP
 *    fetches at 700-1200 ms) so the cadence feels human.
 *  - Two denied beats land mid-flow to deliver the killer trio narrative:
 *    Timeline + Permissions + Sandbox.
 */

export interface DemoStep {
  /** ms after the previous step's *appearance* before showing this one. */
  delay: number;
  /** How long the pending phase lasts before transitioning to final status. */
  durationMs?: number;
  kind: ActionKind;
  toolName: string;
  target: string;
  params: Record<string, unknown>;
  finalStatus?: ActionStatus;
  deniedReason?: string;
  errorMessage?: string;
}

export const DEMO_SERVER_ID = "demo-agent";
export const DEMO_SERVER_NAME = "Demo Agent";

export const DEMO_SCRIPT: DemoStep[] = [
  {
    delay: 400,
    durationMs: 320,
    kind: "fs-read",
    toolName: "list_directory",
    target: "~/Projects/mcp-hub",
    params: { path: "~/Projects/mcp-hub" },
  },
  {
    delay: 800,
    durationMs: 410,
    kind: "fs-read",
    toolName: "read_file",
    target: "~/Projects/mcp-hub/README.md",
    params: { path: "~/Projects/mcp-hub/README.md" },
  },
  {
    delay: 1400,
    durationMs: 940,
    kind: "search",
    toolName: "brave_web_search",
    target: "Tauri 2 capabilities permissions",
    params: { query: "Tauri 2 capabilities permissions", count: 5 },
  },
  {
    delay: 1100,
    durationMs: 1180,
    kind: "http-fetch",
    toolName: "fetch",
    target: "https://tauri.app/v2/reference/capabilities/",
    params: { url: "https://tauri.app/v2/reference/capabilities/" },
  },
  {
    delay: 1500,
    durationMs: 1620,
    kind: "browser-open",
    toolName: "puppeteer_screenshot",
    target: "https://tauri.app/v2/reference/capabilities/",
    params: {
      url: "https://tauri.app/v2/reference/capabilities/",
      selector: "main",
      width: 1280,
      height: 800,
    },
  },
  // -----------------------------------------------------------------
  // Killer narrative beat #1 — agent tries to fetch a host outside its
  // allowed list, MCP Hub denies it at the proxy boundary, Timeline
  // surfaces the denial in the same UI language as a success.
  // -----------------------------------------------------------------
  {
    delay: 1200,
    durationMs: 240,
    kind: "http-fetch",
    toolName: "fetch",
    target: "https://internal.example.com/secrets",
    params: { url: "https://internal.example.com/secrets" },
    finalStatus: "denied",
    deniedReason: "Network blocked by sandbox (host not in allowlist)",
  },
  {
    delay: 900,
    durationMs: 520,
    kind: "fs-write",
    toolName: "edit_file",
    target: "~/Projects/mcp-hub/src-tauri/capabilities/default.json",
    params: {
      path: "~/Projects/mcp-hub/src-tauri/capabilities/default.json",
      edits: [
        {
          oldText: '"core:default"',
          newText: '"core:default", "core:window:set-title"',
        },
      ],
    },
  },
  // Killer narrative beat #2 — write outside granted scope.
  {
    delay: 900,
    durationMs: 220,
    kind: "fs-write",
    toolName: "write_file",
    target: "/etc/hosts",
    params: { path: "/etc/hosts", content: "127.0.0.1 evil.example.com" },
    finalStatus: "denied",
    deniedReason: "Path outside granted fs.write scope (~/Projects)",
  },
  {
    delay: 900,
    durationMs: 740,
    kind: "terminal-exec",
    toolName: "execute_command",
    target: "cargo check -p mcp-hub",
    params: {
      command: "cargo check -p mcp-hub",
      cwd: "~/Projects/mcp-hub/src-tauri",
    },
  },
  {
    delay: 1100,
    durationMs: 380,
    kind: "memory-store",
    toolName: "create_entities",
    target: "Tauri capability research",
    params: {
      entities: [
        {
          name: "Tauri capability research",
          entityType: "research",
          observations: [
            "capabilities/default.json controls runtime permissions",
            "needed core:window:set-title for dynamic titles",
          ],
        },
      ],
    },
  },
];

/**
 * Build the pair of (pending, settled) AgentActions for one demo step.
 * Both carry the **same id**, so the Timeline merges them in place — the
 * card morphs from pending to its terminal state.
 */
export function makeDemoActions(step: DemoStep): {
  pending: AgentAction;
  settled: AgentAction;
} {
  const id = `demo-${cryptoRandomId()}`;
  const base: Omit<AgentAction, "status" | "latencyMs" | "deniedReason" | "error"> =
    {
      id,
      serverId: DEMO_SERVER_ID,
      kind: step.kind,
      toolName: step.toolName,
      target: step.target,
      params: step.params,
      requestId: null,
      timestamp: new Date().toISOString(),
    };

  const pending: AgentAction = {
    ...base,
    status: "pending",
    latencyMs: null,
    deniedReason: null,
    error: null,
  };

  const finalStatus = step.finalStatus ?? "success";
  const settled: AgentAction = {
    ...base,
    timestamp: new Date().toISOString(),
    status: finalStatus,
    latencyMs: step.durationMs ?? 600,
    deniedReason: finalStatus === "denied" ? step.deniedReason ?? null : null,
    error: finalStatus === "failed" ? step.errorMessage ?? null : null,
  };

  return { pending, settled };
}

function cryptoRandomId(): string {
  return (
    Math.random().toString(36).slice(2, 10) +
    Date.now().toString(36).slice(-4)
  );
}
