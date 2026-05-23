import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowDownToLine, Eraser, Pause, Play } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type { LogEntry, LogStream } from "@/types/logs";

type StreamFilter = "all" | LogStream;

const STREAM_TONE: Record<LogStream, string> = {
  stdout: "text-emerald-400",
  stderr: "text-rose-400",
};

interface LogViewerProps {
  entries: LogEntry[];
  serverName: string;
  loading: boolean;
  onClear: () => void;
}

export function LogViewer({
  entries,
  serverName,
  loading,
  onClear,
}: LogViewerProps) {
  const [streamFilter, setStreamFilter] = useState<StreamFilter>("all");
  const [autoScroll, setAutoScroll] = useState(true);
  const [query, setQuery] = useState("");
  const containerRef = useRef<HTMLDivElement | null>(null);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return entries.filter((e) => {
      if (streamFilter !== "all" && e.stream !== streamFilter) return false;
      if (q && !e.message.toLowerCase().includes(q)) return false;
      return true;
    });
  }, [entries, streamFilter, query]);

  // Auto-scroll on new lines, but only if the user hasn't scrolled away.
  useEffect(() => {
    if (!autoScroll) return;
    const el = containerRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [filtered, autoScroll]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Toolbar */}
      <div className="flex items-center gap-2 border-b border-border px-4 py-2">
        <div className="flex items-center gap-1 rounded-md border border-border bg-muted/30 p-0.5">
          {(["all", "stdout", "stderr"] as const).map((s) => (
            <button
              key={s}
              type="button"
              onClick={() => setStreamFilter(s)}
              className={cn(
                "rounded px-2 py-0.5 text-[11px] font-medium uppercase tracking-wide transition-colors",
                streamFilter === s
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {s}
            </button>
          ))}
        </div>

        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Filter lines…"
          className="h-7 flex-1 text-xs"
        />

        <Button
          variant="ghost"
          size="sm"
          onClick={() => setAutoScroll((v) => !v)}
          className="h-7 px-2 text-xs"
          aria-pressed={autoScroll}
          title={autoScroll ? "Pause auto-scroll" : "Resume auto-scroll"}
        >
          {autoScroll ? (
            <>
              <Pause className="size-3" /> Auto
            </>
          ) : (
            <>
              <Play className="size-3" /> Paused
            </>
          )}
        </Button>

        <Button
          variant="ghost"
          size="sm"
          onClick={() => {
            const el = containerRef.current;
            if (el) el.scrollTop = el.scrollHeight;
          }}
          className="h-7 px-2"
          aria-label="Scroll to bottom"
        >
          <ArrowDownToLine className="size-3.5" />
        </Button>

        <Button
          variant="ghost"
          size="sm"
          onClick={onClear}
          className="h-7 px-2 text-xs"
        >
          <Eraser className="size-3" /> Clear
        </Button>
      </div>

      {/* Lines */}
      <div
        ref={containerRef}
        className="relative min-h-0 flex-1 overflow-y-auto bg-[hsl(240_10%_3%)] font-mono text-[11.5px] leading-relaxed"
      >
        {loading ? (
          <div className="flex h-full items-center justify-center text-xs text-muted-foreground">
            Loading logs…
          </div>
        ) : filtered.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-1.5 text-center text-xs text-muted-foreground">
            <span className="font-medium">No log lines yet</span>
            <span className="max-w-xs text-[11px]">
              {serverName} is running but hasn't produced output. Most MCP
              servers stay quiet until a client connects over stdio.
            </span>
          </div>
        ) : (
          <ul className="px-4 py-2">
            {filtered.map((e, i) => (
              <li key={`${e.timestamp}-${i}`} className="flex gap-3 py-0.5">
                <span className="shrink-0 text-muted-foreground/60">
                  {formatTime(e.timestamp)}
                </span>
                <span
                  className={cn(
                    "w-12 shrink-0 text-[10px] uppercase tracking-wider",
                    STREAM_TONE[e.stream],
                  )}
                >
                  {e.stream}
                </span>
                <span className="min-w-0 flex-1 whitespace-pre-wrap break-words text-foreground/90">
                  {e.message}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* Footer */}
      <div className="flex items-center justify-between border-t border-border px-4 py-1.5 text-[11px] text-muted-foreground">
        <span>
          {filtered.length} line{filtered.length === 1 ? "" : "s"}
          {filtered.length !== entries.length && (
            <span className="ml-1 text-muted-foreground/60">
              (filtered from {entries.length})
            </span>
          )}
        </span>
        <span className="font-mono">{serverName}</span>
      </div>
    </div>
  );
}

function formatTime(iso: string): string {
  const d = new Date(iso);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  const ms = String(d.getMilliseconds()).padStart(3, "0");
  return `${hh}:${mm}:${ss}.${ms}`;
}
