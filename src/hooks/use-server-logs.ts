import { useCallback, useEffect, useRef, useState } from "react";

import { api, isTauri, subscribeServerLogs } from "@/lib/tauri";
import type { LogEntry } from "@/types/logs";

const MAX_LIVE_ENTRIES = 2000;

/**
 * Live tail of one server's logs. Combines:
 *   - an initial snapshot fetched from the Rust ring buffer
 *   - a long-lived subscription to the `server-log` Tauri event
 *
 * When `serverId` changes we reset state and re-fetch. The event listener
 * stays mounted for the lifetime of the hook (cheap) and filters by id.
 */
export function useServerLogs(serverId: string | null) {
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [loading, setLoading] = useState(false);

  // Keep the current id in a ref so the global event listener can read the
  // latest value without re-subscribing on every change.
  const currentId = useRef<string | null>(serverId);
  useEffect(() => {
    currentId.current = serverId;
  }, [serverId]);

  // Initial snapshot.
  useEffect(() => {
    if (!serverId) {
      setEntries([]);
      return;
    }
    setLoading(true);
    if (isTauri) {
      api
        .getServerLogs(serverId, 500)
        .then(setEntries)
        .catch(() => setEntries([]))
        .finally(() => setLoading(false));
    } else {
      setEntries(mockLogs(serverId));
      setLoading(false);
    }
  }, [serverId]);

  // Live subscription.
  useEffect(() => {
    if (!isTauri) return;

    let alive = true;
    let unlisten: (() => void) | null = null;

    subscribeServerLogs((entry) => {
      if (!alive) return;
      if (entry.serverId !== currentId.current) return;
      setEntries((prev) => {
        if (prev.length < MAX_LIVE_ENTRIES) return [...prev, entry];
        return [...prev.slice(prev.length - MAX_LIVE_ENTRIES + 1), entry];
      });
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

  const clear = useCallback(async () => {
    if (serverId && isTauri) {
      try {
        await api.clearServerLogs(serverId);
      } catch {
        // Non-fatal — UI clears anyway.
      }
    }
    setEntries([]);
  }, [serverId]);

  return { entries, loading, clear };
}

function mockLogs(serverId: string): LogEntry[] {
  const now = Date.now();
  const mk = (offset: number, stream: "stdout" | "stderr", message: string): LogEntry => ({
    serverId,
    stream,
    message,
    timestamp: new Date(now - offset).toISOString(),
  });
  return [
    mk(5000, "stderr", "MCP server starting…"),
    mk(4500, "stderr", "Listening on stdio"),
    mk(3000, "stdout", '{"jsonrpc":"2.0","result":{"protocolVersion":"2024-11-05"},"id":0}'),
    mk(2000, "stderr", "Awaiting client tools/list"),
  ];
}
