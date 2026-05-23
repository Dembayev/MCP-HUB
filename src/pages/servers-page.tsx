import { Plus, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/servers/empty-state";
import { ServerList } from "@/components/servers/server-list";
import { useMcpServers } from "@/hooks/use-mcp-servers";

export function ServersPage() {
  const { servers, loading, error, refresh, start, stop, remove } =
    useMcpServers();

  const runningCount = servers.filter((s) => s.status === "running").length;

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between gap-4 border-b border-border px-6 py-4">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">Servers</h1>
          <p className="text-xs text-muted-foreground">
            {servers.length} installed · {runningCount} running
          </p>
        </div>

        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => void refresh()}
            disabled={loading}
            aria-label="Refresh"
          >
            <RefreshCw className={loading ? "animate-spin" : ""} />
          </Button>
          <Button>
            <Plus className="size-3.5" />
            Install server
          </Button>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-6 animate-fade-in">
        {error ? (
          <div className="rounded-lg border border-destructive/40 bg-destructive/10 p-4 text-sm text-destructive">
            {error}
          </div>
        ) : servers.length === 0 && !loading ? (
          <EmptyState />
        ) : (
          <ServerList
            servers={servers}
            onStart={(id) => void start(id)}
            onStop={(id) => void stop(id)}
            onRemove={(id) => void remove(id)}
          />
        )}
      </div>
    </div>
  );
}
