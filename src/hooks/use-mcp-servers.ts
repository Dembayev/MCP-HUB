import { useCallback, useEffect, useMemo, useState } from "react";

import { api, isTauri, subscribeServerStatus } from "@/lib/tauri";
import type { InstallServerRequest, McpServer } from "@/types/mcp";
import type { MarketplaceEntry } from "@/types/marketplace";

/**
 * Browser-mode fallback so designers can iterate on the UI with `npm run dev`
 * without a running Tauri shell. Replaced with real data the moment we're
 * inside a Tauri window.
 */
const MOCK_SERVERS: McpServer[] = [
  {
    id: "mock-filesystem",
    name: "Filesystem",
    description: "Browse, read, and write files on your machine.",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-filesystem", "~/Projects"],
    env: {},
    transport: "stdio",
    status: "running",
    installedAt: new Date(Date.now() - 1000 * 60 * 60 * 24 * 3).toISOString(),
    updatedAt: new Date(Date.now() - 1000 * 60 * 5).toISOString(),
    version: "0.4.1",
    source: "registry",
    iconUrl: null,
  },
  {
    id: "mock-github",
    name: "GitHub",
    description: "Read issues, PRs, and repo metadata.",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-github"],
    env: { GITHUB_TOKEN: "***" },
    transport: "stdio",
    status: "stopped",
    installedAt: new Date(Date.now() - 1000 * 60 * 60 * 24 * 10).toISOString(),
    updatedAt: new Date(Date.now() - 1000 * 60 * 60).toISOString(),
    version: "0.6.0",
    source: "registry",
    iconUrl: null,
  },
  {
    id: "mock-postgres",
    name: "Postgres",
    description: "Query a local Postgres instance.",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-postgres"],
    env: { DATABASE_URL: "postgres://localhost/dev" },
    transport: "stdio",
    status: "crashed",
    installedAt: new Date(Date.now() - 1000 * 60 * 60 * 24 * 30).toISOString(),
    updatedAt: new Date(Date.now() - 1000 * 60 * 60 * 6).toISOString(),
    version: "0.3.2",
    source: "manual",
    iconUrl: null,
  },
];

const MARKETPLACE_SOURCE_PREFIX = "marketplace:";

/** Stable `source` value we write into the DB so we can dedupe marketplace installs. */
export function marketplaceSourceFor(entryId: string): string {
  return `${MARKETPLACE_SOURCE_PREFIX}${entryId}`;
}

interface UseMcpServersResult {
  servers: McpServer[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  start: (id: string) => Promise<void>;
  stop: (id: string) => Promise<void>;
  remove: (id: string) => Promise<void>;
  install: (entry: MarketplaceEntry) => Promise<McpServer>;
  installedMarketplaceIds: Set<string>;
}

export function useMcpServers(): UseMcpServersResult {
  const [servers, setServers] = useState<McpServer[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setError(null);
    try {
      if (!isTauri) {
        setServers(MOCK_SERVERS);
        return;
      }
      const data = await api.listServers();
      setServers(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Live status updates. Cheaper than polling and snappier than waiting
  // for the user to hit refresh — when the supervisor task notices a crash,
  // the grid reflects it within a frame.
  useEffect(() => {
    if (!isTauri) return;
    let alive = true;
    let unlisten: (() => void) | null = null;

    subscribeServerStatus((change) => {
      if (!alive) return;
      setServers((prev) =>
        prev.map((srv) =>
          srv.id === change.id ? { ...srv, status: change.status } : srv,
        ),
      );
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

  const start = useCallback(
    async (id: string) => {
      if (!isTauri) {
        setServers((s) =>
          s.map((srv) => (srv.id === id ? { ...srv, status: "running" } : srv)),
        );
        return;
      }
      await api.startServer(id);
      await refresh();
    },
    [refresh],
  );

  const stop = useCallback(
    async (id: string) => {
      if (!isTauri) {
        setServers((s) =>
          s.map((srv) => (srv.id === id ? { ...srv, status: "stopped" } : srv)),
        );
        return;
      }
      await api.stopServer(id);
      await refresh();
    },
    [refresh],
  );

  const remove = useCallback(
    async (id: string) => {
      if (!isTauri) {
        setServers((s) => s.filter((srv) => srv.id !== id));
        return;
      }
      await api.removeServer(id);
      await refresh();
    },
    [refresh],
  );

  const install = useCallback(
    async (entry: MarketplaceEntry): Promise<McpServer> => {
      const request: InstallServerRequest = {
        name: entry.name,
        description: entry.description,
        command: entry.command,
        args: entry.args,
        env: {},
        transport: entry.transport,
        version: entry.version,
        source: marketplaceSourceFor(entry.id),
        iconUrl: null,
        // Forward consented permissions to the backend so the sandbox
        // layer can enforce them on first start.
        permissions: entry.permissions.map((p) => ({
          scope: p.scope,
          target: p.target ?? null,
          reason: p.reason,
        })),
      };

      if (!isTauri) {
        const now = new Date().toISOString();
        const stub: McpServer = {
          id: `mock-${entry.id}-${Date.now()}`,
          name: request.name,
          description: request.description ?? "",
          command: request.command,
          args: request.args ?? [],
          env: request.env ?? {},
          transport: request.transport ?? "stdio",
          status: "stopped",
          installedAt: now,
          updatedAt: now,
          version: request.version ?? null,
          source: request.source ?? null,
          iconUrl: null,
        };
        setServers((s) => [...s, stub]);
        return stub;
      }

      const created = await api.installServer(request);
      await refresh();
      return created;
    },
    [refresh],
  );

  const installedMarketplaceIds = useMemo(() => {
    const set = new Set<string>();
    for (const s of servers) {
      if (s.source?.startsWith(MARKETPLACE_SOURCE_PREFIX)) {
        set.add(s.source.slice(MARKETPLACE_SOURCE_PREFIX.length));
      }
    }
    return set;
  }, [servers]);

  return {
    servers,
    loading,
    error,
    refresh,
    start,
    stop,
    remove,
    install,
    installedMarketplaceIds,
  };
}
