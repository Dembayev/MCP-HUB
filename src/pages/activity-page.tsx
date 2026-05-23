import { useEffect, useState } from "react";
import { Activity } from "lucide-react";

import { LogViewer } from "@/components/activity/log-viewer";
import { ServerPicker } from "@/components/activity/server-picker";
import { useMcpServers } from "@/hooks/use-mcp-servers";
import { useServerLogs } from "@/hooks/use-server-logs";

export function ActivityPage() {
  const { servers } = useMcpServers();
  const [selectedId, setSelectedId] = useState<string | null>(null);

  // Auto-select the first running server when nothing is picked yet, or
  // fall back to the first installed server. Keeps the page useful on
  // first navigation.
  useEffect(() => {
    if (selectedId && servers.some((s) => s.id === selectedId)) return;
    const next =
      servers.find((s) => s.status === "running") ?? servers[0] ?? null;
    setSelectedId(next ? next.id : null);
  }, [servers, selectedId]);

  const selected = servers.find((s) => s.id === selectedId) ?? null;
  const { entries, loading, clear } = useServerLogs(selectedId);

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-border px-6 py-4">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">Activity</h1>
          <p className="text-xs text-muted-foreground">
            Live stdout and stderr from every running MCP server.
          </p>
        </div>
        <div className="flex items-center gap-2 rounded-md border border-border bg-muted/30 px-2 py-1 text-[11px] text-muted-foreground">
          <Activity className="size-3" />
          {servers.filter((s) => s.status === "running").length} running
        </div>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[260px_1fr]">
        <aside className="min-h-0 overflow-y-auto border-r border-border bg-card/40">
          <ServerPicker
            servers={servers}
            selectedId={selectedId}
            onSelect={setSelectedId}
          />
        </aside>

        <section className="min-h-0">
          {selected ? (
            <LogViewer
              entries={entries}
              serverName={selected.name}
              loading={loading}
              onClear={() => void clear()}
            />
          ) : (
            <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
              Install a server to start watching activity.
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
