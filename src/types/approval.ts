/**
 * Runtime approval flow types — mirror `src-tauri/src/security/approvals.rs`.
 *
 * The popup that gates MCP requests on missing permissions. Third verb of
 * "See → Replay → Approve".
 */

import type { ActionKind } from "@/types/actions";

export type RiskLevel = "low" | "medium" | "high";

/**
 * Sent from backend via `approval-requested` Tauri event. Carries everything
 * the popup needs to render and route the decision back via `resolve_approval`.
 */
export interface ApprovalRequest {
  id: string;
  serverId: string;
  serverName: string;
  tool: string;
  kind: ActionKind;
  target: string | null;
  /** Scope token that "Always Allow" would persist (e.g. "fs.write"). */
  scope: string;
  risk: RiskLevel;
  /** RFC 3339 timestamp. */
  requestedAt: string;
}

/**
 * User's choice. Snake_case on the wire — matches the Rust `#[serde(rename_all = "snake_case")]`.
 */
export type ApprovalDecision = "allow_once" | "allow_session" | "deny";
