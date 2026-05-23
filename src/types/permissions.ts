import type { PermissionScope } from "./marketplace";

/**
 * Persisted permission grant fetched from the Rust side. Mirrors
 * `db::permissions::PersistedPermission`.
 */
export interface PersistedPermission {
  id: number;
  serverId: string;
  scope: PermissionScope | string;
  target: string | null;
  reason: string | null;
  granted: boolean;
  /** ISO 8601 — null if never granted. */
  grantedAt: string | null;
}
