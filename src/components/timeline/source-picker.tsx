import { Sparkles } from "lucide-react";

import { StatusDot } from "@/components/servers/status-dot";
import { cn } from "@/lib/utils";
import { DEMO_SERVER_ID, DEMO_SERVER_NAME } from "@/data/demo-script";
import type { McpServer } from "@/types/mcp";

interface SourcePickerProps {
  servers: McpServer[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}

/**
 * Source picker for the Timeline page. Lists installed servers plus a
 * special "Demo Agent" entry at the top so users can switch into a
 * showcase session with one click.
 */
export function SourcePicker({
  servers,
  selectedId,
  onSelect,
}: SourcePickerProps) {
  return (
    <ul className="flex flex-col gap-0.5 p-2">
      <li>
        <button
          type="button"
          onClick={() => onSelect(DEMO_SERVER_ID)}
          className={cn(
            "flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-sm transition-colors",
            selectedId === DEMO_SERVER_ID
              ? "bg-primary/10 text-foreground ring-1 ring-primary/30"
              : "text-muted-foreground hover:bg-accent/40 hover:text-foreground",
          )}
        >
          <Sparkles className="size-3.5 text-primary" />
          <span className="min-w-0 flex-1 truncate">{DEMO_SERVER_NAME}</span>
          <span className="shrink-0 text-[10px] uppercase tracking-wide text-primary/70">
            demo
          </span>
        </button>
      </li>

      {servers.length > 0 && (
        <li
          className="px-2.5 pb-1 pt-3 text-[10px] font-medium uppercase tracking-wider text-muted-foreground/60"
          aria-hidden
        >
          Installed
        </li>
      )}

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
