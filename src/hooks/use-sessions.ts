/**
 * Session trace data hooks for the Timeline tab.
 *
 * Live-refresh strategy is simple polling at 1 Hz:
 *
 * - `useSessions()` polls `list_sessions` every 1 s and exposes the list.
 * - `useSessionTrace(id)` polls `get_session(id)` every 1 s when an id is
 *   selected. While a session is still being appended to by the writer task
 *   (step 2), each poll reads the latest state from disk.
 *
 * Polling is cheap (a session file is bounded in size; the reader sorts in
 * memory) and gives us "live refresh" without bringing in `notify` or a
 * file-watcher event channel. Upgrading to push-based events is a v0.2
 * optimization — out of scope here.
 *
 * When running outside Tauri (plain `vite dev`), the hooks short-circuit to
 * an empty / null state so the UI still renders without errors.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import { api, isTauri } from "@/lib/tauri";
import type { SessionFile, SessionSummary } from "@/types/session";

const POLL_INTERVAL_MS = 1000;

export interface UseSessionsResult {
  sessions: SessionSummary[];
  loading: boolean;
  error: string | null;
  /** Force-refresh on demand (e.g. after seedDemoSession completes). */
  refresh: () => Promise<void>;
  /** Convenience action — wraps api.seedDemoSession + refresh. */
  seedDemo: () => Promise<string | null>;
}

export function useSessions(): UseSessionsResult {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  // Prevents a stale fetch from overwriting a fresher one after unmount.
  const mountedRef = useRef(true);

  const fetchOnce = useCallback(async () => {
    if (!isTauri) {
      setSessions([]);
      setLoading(false);
      return;
    }
    try {
      const list = await api.listSessions();
      if (!mountedRef.current) return;
      setSessions(list);
      setError(null);
    } catch (err) {
      if (!mountedRef.current) return;
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (mountedRef.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    void fetchOnce();
    const handle = setInterval(() => {
      void fetchOnce();
    }, POLL_INTERVAL_MS);
    return () => {
      mountedRef.current = false;
      clearInterval(handle);
    };
  }, [fetchOnce]);

  const seedDemo = useCallback(async () => {
    if (!isTauri) return null;
    try {
      const id = await api.seedDemoSession();
      await fetchOnce();
      return id;
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      return null;
    }
  }, [fetchOnce]);

  return { sessions, loading, error, refresh: fetchOnce, seedDemo };
}

// ---------------------------------------------------------------------------

export interface UseSessionTraceResult {
  trace: SessionFile | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

/**
 * Cheap fingerprint comparison — covers every change scenario the writer
 * task can produce (new action appended, session finalized) without
 * paying for a deep deserialize-and-compare on every poll.
 */
function tracesEqual(a: SessionFile, b: SessionFile): boolean {
  if (a.actions.length !== b.actions.length) return false;
  if (a.session.ended_at !== b.session.ended_at) return false;
  const lastA = a.actions[a.actions.length - 1];
  const lastB = b.actions[b.actions.length - 1];
  return (lastA?.id ?? null) === (lastB?.id ?? null);
}

/**
 * Loads (and polls) the full trace for one session id. Pass `null` to
 * disable — the hook short-circuits and exposes a null trace.
 */
export function useSessionTrace(id: string | null): UseSessionTraceResult {
  const [trace, setTrace] = useState<SessionFile | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);

  const fetchOnce = useCallback(async () => {
    if (!isTauri || !id) {
      setTrace(null);
      setLoading(false);
      return;
    }
    try {
      const file = await api.getSession(id);
      if (!mountedRef.current) return;
      // Reference-stable update: only swap state when the trace actually
      // changed (new action appended, session finalized, …). Without this,
      // every poll produces a new object identity, which would reset every
      // downstream consumer that depends on the actions array — most
      // visibly useReplay, where it kept restarting playback every second.
      setTrace((prev) => (prev && tracesEqual(prev, file) ? prev : file));
      setError(null);
    } catch (err) {
      if (!mountedRef.current) return;
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (mountedRef.current) setLoading(false);
    }
  }, [id]);

  useEffect(() => {
    mountedRef.current = true;
    if (!id) {
      setTrace(null);
      return () => {
        mountedRef.current = false;
      };
    }
    setLoading(true);
    void fetchOnce();
    const handle = setInterval(() => {
      void fetchOnce();
    }, POLL_INTERVAL_MS);
    return () => {
      mountedRef.current = false;
      clearInterval(handle);
    };
  }, [id, fetchOnce]);

  return { trace, loading, error, refresh: fetchOnce };
}
