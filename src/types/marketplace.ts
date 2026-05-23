/**
 * Types for the curated MCP marketplace. These mirror the product spec:
 * trust badges, permission scopes (`fs.read`, `internet`, etc.), and the
 * minimum metadata required for a card.
 */

import type { Transport } from "./mcp";

export type TrustBadge =
  | "verified"           // Reviewed by MCP Hub maintainers
  | "community-trusted"  // Widely used, vetted by the community
  | "experimental"       // Useful but new — may break
  | "unsafe";            // Known issues or unaudited

export type PermissionScope =
  | "fs.read"
  | "fs.write"
  | "internet"
  | "browser"
  | "terminal"
  | "clipboard"
  | "microphone"
  | "env.read"
  | "exec";

export interface RequiredPermission {
  scope: PermissionScope;
  /** Human-readable target ("~/Documents", "api.github.com", ...) */
  target?: string;
  /** Short justification surfaced in the install dialog. */
  reason: string;
}

export type Category =
  | "filesystem"
  | "developer-tools"
  | "web"
  | "data"
  | "productivity"
  | "ai"
  | "other";

export interface MarketplaceEntry {
  /** Stable identifier used to dedupe against installed servers. */
  id: string;
  name: string;
  author: string;
  description: string;
  category: Category;
  trust: TrustBadge;
  /** GitHub repo URL or homepage. */
  homepage: string;
  /** Approximate GitHub star count at time of curation. */
  stars: number;
  /** Latest known version. */
  version: string;
  /** Approximate install size for the "What you're getting" line. */
  installSize: string;
  transport: Transport;
  command: string;
  args: string[];
  /** Env vars the user must provide; keys only, values prompted on install. */
  requiredEnv?: string[];
  permissions: RequiredPermission[];
}
