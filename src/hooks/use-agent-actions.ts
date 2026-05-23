import { useCallback, useEffect, useRef, useState } from "react";

import {
  makeDemoActions,
  DEMO_SCRIPT,
  DEMO_SERVER_ID,
} from "@/data/demo-script";
import { api, isTauri, subscribeAgentActions } from "@/lib/tauri";
import type { AgentAction } from "@/types/actions";

const MAX_ACTIONS = 500;

/**
 * Merge an incoming action into the existing list. If we already have a
 * card with the same id, replace it in place (lifecycle update). Otherwise
 * append, bounded by `MAX_ACTIONS`.
 */
function mergeOrAppend(prev: AgentAction[], incoming: AgentAction): AgentAction[] {
  const idx = prev.findIndex((a) => a.id === incoming.id);
  if (idx >= 0) {
    const next = prev.slice();
    next[idx] = incoming;
    return next;
  }
  if (prev.length < MAX_ACTIONS) return [...prev, incoming];
  return [...prev.slice(prev.length - MAX_ACTIONS + 1), incoming];
}

interface UseAgentActionsResult {
  actions: AgentAction[];
  loading: boolean;
  demoRunning: boolean;
  startDemo: () => void;
  stopDemo: () => void;
  clear: () => Promise<void>;
}

/**
 * Live feed of agent actions for a given server, with a built-in Demo Mode
 * for showcase/recording sessions. Real and demo actions render through the
 * same UI so the Timeline page doesn't fork.
 *
 *  - When `serverId === DEMO_SERVER_ID`, the hook only fires synthetic
 *    actions from `DEMO_SCRIPT`. No backend events flow.
 *  - When `serverId` is null, we show nothing (the page renders a CTA).
 *  - Otherwise we fetch the ring buffer once and tail the live event stream.
 */
export function useAgentActions(serverId: string | null): UseAgentActionsResult {
  const [actions, setActions] = useState<AgentAction[]>([]);
  const [loading, setLoading] = useState(false);
  const [demoRunning, setDemoRunning] = useState(false);

  const currentServerId = useRef<string | null>(serverId);
  useEffect(() => {
    currentServerId.current = serverId;
  }, [serverId]);

  // --- Real mode: initial snapshot + live tail. -----------------------------
  useEffect(() => {
    if (!serverId || serverId === DEMO_SERVER_ID) {
      setActions([]);
      return;
    }
    setLoading(true);
    if (isTauri) {
      api
        .getServerActions(serverId, 200)
        .then(setActions)
        .catch(() => setActions([]))
        .finally(() => setLoading(false));
    } else {
      setActions([]);
      setLoading(false);
    }
  }, [serverId]);

  useEffect(() => {
    if (!isTauri) return;
    let alive = true;
    let unlisten: (() => void) | null = null;

    subscribeAgentActions((action) => {
      if (!alive) return;
      if (currentServerId.current === DEMO_SERVER_ID) return;
      if (action.serverId !== currentServerId.current) return;
      setActions((prev) => mergeOrAppend(prev, action));
    }).then((fn) => {
      if (!alive) {
        fn();
        return;
      }
      unlisten = fn;
    });

    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

  // --- Demo mode: timer-driven synthetic actions. ---------------------------
  const demoTimers = useRef<ReturnType<typeof setTimeout>[]>([]);

  const stopDemo = useCallback(() => {
    demoTimers.current.forEach(clearTimeout);
    demoTimers.current = [];
    setDemoRunning(false);
  }, []);

  const startDemo = useCallback(() => {
    stopDemo();
    setActions([]);
    setDemoRunning(true);

    let appearance = 0;
    DEMO_SCRIPT.forEach((step, i) => {
      appearance += step.delay;
      const duration = step.durationMs ?? 600;
      const completion = appearance + duration;

      const { pending, settled } = makeDemoActions(step);

      const pendingTimer = setTimeout(() => {
        setActions((prev) => mergeOrAppend(prev, pending));
      }, appearance);
      demoTimers.current.push(pendingTimer);

      const settleTimer = setTimeout(() => {
        setActions((prev) => mergeOrAppend(prev, settled));
        if (i === DEMO_SCRIPT.length - 1) {
          setDemoRunning(false);
        }
      }, completion);
      demoTimers.current.push(settleTimer);
    });
  }, [stopDemo]);

  useEffect(() => stopDemo, [stopDemo]);

  const clear = useCallback(async () => {
    stopDemo();
    if (serverId && serverId !== DEMO_SERVER_ID && isTauri) {
      try {
        await api.clearServerActions(serverId);
      } catch {
        // Non-fatal.
      }
    }
    setActions([]);
  }, [serverId, stopDemo]);

  return { actions, loading, demoRunning, startDemo, stopDemo, clear };
}
