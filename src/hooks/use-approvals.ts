/**
 * Approval queue hook — drains incoming `approval-requested` Tauri events
 * into an in-memory FIFO and exposes the head + a `resolve` action.
 *
 * Why FIFO: if the agent fires multiple gated requests in quick succession
 * (e.g. read 3 files where none of `fs.read` is granted), each generates a
 * separate event. The modal can only show one at a time; the rest queue
 * until the user resolves them in order.
 *
 * Outside of Tauri (plain `vite dev`), the hook returns an empty state and
 * never subscribes — keeps the UI working in browser-only design mode.
 */

import { useCallback, useEffect, useState } from "react";

import { api, isTauri, subscribeApprovalRequests } from "@/lib/tauri";
import type { ApprovalDecision, ApprovalRequest } from "@/types/approval";

export interface UseApprovalsResult {
  /** Head of the queue, or null when nothing is pending. */
  current: ApprovalRequest | null;
  /** Total pending (including the head). For UI badges. */
  pending: number;
  /** Resolve the head approval and advance to the next. */
  resolve: (decision: ApprovalDecision) => Promise<void>;
}

export function useApprovals(): UseApprovalsResult {
  const [queue, setQueue] = useState<ApprovalRequest[]>([]);

  // Subscribe to approval-requested events. Only inside Tauri.
  useEffect(() => {
    if (!isTauri) return;
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    subscribeApprovalRequests((req) => {
      if (cancelled) return;
      setQueue((prev) => {
        // De-dupe in case the backend re-emits (defensive — shouldn't happen
        // under normal flow, but is cheap insurance).
        if (prev.some((p) => p.id === req.id)) return prev;
        return [...prev, req];
      });
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const resolve = useCallback(
    async (decision: ApprovalDecision) => {
      const head = queue[0];
      if (!head) return;
      // Optimistically drop from the queue; on error we leave a console
      // breadcrumb but DON'T re-enqueue (the backend treats unknown ids as
      // already-resolved). The proxy will fail-safe deny via dropped
      // channel if anything went wrong.
      setQueue((prev) => prev.slice(1));
      try {
        if (isTauri) {
          await api.resolveApproval(head.id, decision);
        }
      } catch (err) {
        // eslint-disable-next-line no-console
        console.warn("resolve_approval failed", head.id, err);
      }
    },
    [queue],
  );

  return {
    current: queue[0] ?? null,
    pending: queue.length,
    resolve,
  };
}
