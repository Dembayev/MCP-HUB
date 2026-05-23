import { Boxes } from "lucide-react";

import { StatusDot } from "@/components/servers/status-dot";
import { cn } from "@/lib/utils";
import type { McpServer } from "@/types/mcp";

interface ServerPickerProps {
  servers: McpServer[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}

export function ServerPicker({
  servers,
  selectedId,
  onSelect,
}: ServerPickerProps) {
  if (servers.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-6 text-center">
        <div className="flex size-10 items-center justify-center rounded-xl border border-border bg-muted/30">
          <Boxes className="size-4 text-muted-foreground" />
        </div>
        <p className="text-xs text-muted-foreground">
          Install a server from the marketplace to start seeing activity.
        </p>
      </div>
    );
  }

  return (
    <ul className="flex flex-col gap-0.5 p-2">
      {servers.map((s) => {
        const active = s.id === selectedId;
        return (
          <li key={s.id}>
            <button
              type="button"
              onClick={() => onSelect(s.id)}
              className={cn(
                "flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-sm transition-colors",
                active
                  ? "bg-accent text-foreground"
                  : "text-muted-foreground hover:bg-accent/40 hover:text-foreground",
              )}
            >
              <StatusDot status={s.status} />
              <span className="min-w-0 flex-1 truncate">{s.name}</span>
              <span
                className={cn(
                  "shrink-0 text-[10px] uppercase tracking-wide",
                  active ? "text-muted-foreground" : "text-muted-foreground/70",
                )}
              >
                {s.status}
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
