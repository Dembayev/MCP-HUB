import { useEffect, useRef, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { Eraser, GitBranch, Sparkles, Square } from "lucide-react";

import { SourcePicker } from "@/components/timeline/source-picker";
import { TimelineCard } from "@/components/timeline/timeline-card";
import { Button } from "@/components/ui/button";
import {
  DEMO_SCRIPT,
  DEMO_SERVER_ID,
  DEMO_SERVER_NAME,
} from "@/data/demo-script";
import { useAgentActions } from "@/hooks/use-agent-actions";
import { useMcpServers } from "@/hooks/use-mcp-servers";

const DEMO_TOTAL_MS = DEMO_SCRIPT.reduce((acc, s) => acc + s.delay, 0);

export function TimelinePage() {
  const { servers } = useMcpServers();
  const [selectedId, setSelectedId] = useState<string | null>(DEMO_SERVER_ID);
  const { actions, demoRunning, startDemo, stopDemo, clear } =
    useAgentActions(selectedId);

  const isDemo = selectedId === DEMO_SERVER_ID;
  const selectedServer = servers.find((s) => s.id === selectedId);
  const sourceName = isDemo
    ? DEMO_SERVER_NAME
    : selectedServer?.name ?? "—";

  // Auto-scroll the feed as new cards arrive.
  const feedRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    const el = feedRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [actions]);

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="flex items-center justify-between gap-4 border-b border-border px-6 py-4">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">Timeline</h1>
          <p className="text-xs text-muted-foreground">
            See everything your AI agent does — step by step.
          </p>
        </div>

        <div className="flex items-center gap-2">
          {isDemo ? (
            demoRunning ? (
              <Button variant="secondary" size="sm" onClick={stopDemo}>
                <Square className="size-3.5" /> Stop demo
              </Button>
            ) : (
              <Button size="sm" onClick={startDemo}>
                <Sparkles className="size-3.5" /> Start demo session
              </Button>
            )
          ) : null}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void clear()}
            className="text-xs"
          >
            <Eraser className="size-3" /> Clear
          </Button>
        </div>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[240px_1fr]">
        {/* Source list */}
        <aside className="min-h-0 overflow-y-auto border-r border-border bg-card/40">
          <SourcePicker
            servers={servers}
            selectedId={selectedId}
            onSelect={setSelectedId}
          />
        </aside>

        {/* Feed */}
        <section className="flex min-h-0 flex-col">
          {/* Subtle banner explaining the source. */}
          <div className="flex items-center gap-2 border-b border-border bg-card/30 px-6 py-2 text-[11px] text-muted-foreground">
            {isDemo ? (
              <>
                <Sparkles className="size-3 text-primary" />
                <span>
                  Demo session — synthetic agent actions, ~
                  {Math.round(DEMO_TOTAL_MS / 1000)}s flow.
                </span>
              </>
            ) : (
              <>
                <GitBranch className="size-3" />
                <span>
                  Streaming live from{" "}
                  <span className="font-mono text-foreground/80">
                    {sourceName}
                  </span>
                  . Real-time tool capture lights up once MCP Hub runs as a
                  proxy between your AI client and the server.
                </span>
              </>
            )}
          </div>

          <div
            ref={feedRef}
            className="min-h-0 flex-1 overflow-y-auto px-6 py-6"
          >
            {actions.length === 0 ? (
              <EmptyFeed isDemo={isDemo} onStartDemo={startDemo} />
            ) : (
              <ol className="relative">
                <AnimatePresence initial={false}>
                  {actions.map((a, i) => (
                    <TimelineCard
                      key={a.id}
                      action={a}
                      showConnector={i > 0}
                    />
                  ))}
                </AnimatePresence>
              </ol>
            )}
          </div>

          {/* Footer */}
          <div className="flex items-center justify-between border-t border-border px-6 py-1.5 text-[11px] text-muted-foreground">
            <span>
              {actions.length} action{actions.length === 1 ? "" : "s"}
            </span>
            <span className="font-mono">{sourceName}</span>
          </div>
        </section>
      </div>
    </div>
  );
}

function EmptyFeed({
  isDemo,
  onStartDemo,
}: {
  isDemo: boolean;
  onStartDemo: () => void;
}) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
      <div className="flex size-12 items-center justify-center rounded-2xl border border-border bg-muted/30">
        <Sparkles className="size-5 text-muted-foreground" />
      </div>
      <div className="max-w-sm space-y-1">
        <h2 className="text-base font-semibold">No actions yet</h2>
        <p className="text-sm text-muted-foreground">
          {isDemo
            ? "Press Start demo session to watch a realistic agent flow render in real time."
            : "Once this server handles tool calls, they'll appear here as semantic steps — file reads, fetches, shell commands."}
        </p>
      </div>
      {isDemo && (
        <Button onClick={onStartDemo} className="mt-2">
          <Sparkles className="size-3.5" /> Start demo session
        </Button>
      )}
    </div>
  );
}
