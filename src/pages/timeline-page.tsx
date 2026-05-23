import { useEffect, useMemo, useState } from "react";
import { Activity, FileText, Sparkles, Terminal } from "lucide-react";

import { ActionRow } from "@/components/timeline/action-row";
import { OutcomeDot } from "@/components/timeline/outcome-badge";
import { ReplayControls } from "@/components/timeline/replay-controls";
import { Scrubber } from "@/components/timeline/scrubber";
import { Button } from "@/components/ui/button";
import { useReplay } from "@/hooks/use-replay";
import { useSessions, useSessionTrace } from "@/hooks/use-sessions";
import { cn } from "@/lib/utils";
import type { SessionFile, SessionSummary } from "@/types/session";

/**
 * Timeline tab — the persistent trace browser. Left sidebar lists sessions
 * found in `<data_dir>/sessions/*.ndjson`, main area renders the selected
 * session as a dense action stream (DevTools-style).
 *
 * Step 3 scope: read-only. No replay engine, no analytics, no graphs.
 * Just "session appears in UI automatically; actions render in seq order;
 * live refresh works without restart; can open a trace and scroll it".
 */
export function TimelinePage() {
  const { sessions, loading, error, seedDemo } = useSessions();
  const [selectedId, setSelectedId] = useState<string | null>(null);

  // Auto-select the newest session whenever the list arrives and we don't
  // already have a valid selection. Means "open the app → see something".
  useEffect(() => {
    if (selectedId && sessions.some((s) => s.id === selectedId)) return;
    setSelectedId(sessions[0]?.id ?? null);
  }, [sessions, selectedId]);

  const selectedSummary = sessions.find((s) => s.id === selectedId) ?? null;
  const { trace, error: traceError } = useSessionTrace(selectedId);

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between gap-4 border-b border-border px-6 py-4">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">Timeline</h1>
          <p className="text-xs text-muted-foreground">
            Every action your AI agents took — sorted, scrollable, replayable.
          </p>
        </div>

        <div className="flex items-center gap-2">
          <Button
            variant="secondary"
            size="sm"
            onClick={() => void seedDemo()}
            className="text-xs"
          >
            <Sparkles className="size-3.5" /> Seed sample trace
          </Button>
        </div>
      </header>

      {error && (
        <div className="border-b border-destructive/30 bg-destructive/10 px-6 py-2 text-xs text-destructive">
          Failed to load sessions: {error}
        </div>
      )}

      <div className="grid min-h-0 flex-1 grid-cols-[280px_1fr]">
        <aside className="min-h-0 overflow-y-auto border-r border-border bg-card/40">
          <SessionList
            sessions={sessions}
            loading={loading}
            selectedId={selectedId}
            onSelect={setSelectedId}
            onSeed={() => void seedDemo()}
          />
        </aside>

        <section className="flex min-h-0 flex-col">
          {selectedSummary && trace ? (
            <TraceView summary={selectedSummary} trace={trace} />
          ) : selectedSummary ? (
            <LoadingTrace />
          ) : (
            <EmptyTrace onSeed={() => void seedDemo()} hasError={Boolean(traceError)} />
          )}
        </section>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Session list (left sidebar)
// ---------------------------------------------------------------------------

function SessionList({
  sessions,
  loading,
  selectedId,
  onSelect,
  onSeed,
}: {
  sessions: SessionSummary[];
  loading: boolean;
  selectedId: string | null;
  onSelect: (id: string) => void;
  onSeed: () => void;
}) {
  if (loading && sessions.length === 0) {
    return (
      <div className="flex h-full items-center justify-center p-6 text-xs text-muted-foreground">
        Loading sessions…
      </div>
    );
  }

  if (sessions.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-6 text-center">
        <div className="flex size-10 items-center justify-center rounded-xl border border-border bg-muted/30">
          <FileText className="size-5 text-muted-foreground" />
        </div>
        <div className="space-y-1">
          <div className="text-sm font-medium">No sessions yet</div>
          <p className="text-xs text-muted-foreground">
            Sessions appear here when MCP Hub proxies an AI client. Drop a sample to explore.
          </p>
        </div>
        <Button size="sm" variant="outline" onClick={onSeed} className="text-xs">
          <Sparkles className="size-3.5" /> Seed sample
        </Button>
      </div>
    );
  }

  return (
    <ul className="flex flex-col">
      {sessions.map((s) => (
        <SessionListItem
          key={s.id}
          session={s}
          active={s.id === selectedId}
          onSelect={() => onSelect(s.id)}
        />
      ))}
    </ul>
  );
}

function SessionListItem({
  session,
  active,
  onSelect,
}: {
  session: SessionSummary;
  active: boolean;
  onSelect: () => void;
}) {
  const truncated = session.status === "truncated";
  const denied = session.deniedCount > 0;
  return (
    <li>
      <button
        type="button"
        onClick={onSelect}
        className={cn(
          "flex w-full items-start gap-2 border-l-2 border-transparent px-3 py-2.5 text-left transition-colors",
          active
            ? "border-primary bg-card text-foreground"
            : "hover:bg-card/60 text-foreground/90",
        )}
      >
        <Activity className="mt-0.5 size-3.5 shrink-0 text-muted-foreground/70" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate text-[13px] font-medium">
              {session.serverName}
            </span>
            {truncated && (
              <span
                className="size-1.5 shrink-0 rounded-full bg-amber-400"
                title="session truncated (no end record)"
              />
            )}
            {denied && (
              <span
                className="size-1.5 shrink-0 rounded-full bg-destructive shadow-[0_0_6px_rgba(248,113,113,0.6)]"
                title={`${session.deniedCount} denied`}
              />
            )}
          </div>
          <div className="mt-0.5 flex items-center gap-2 text-[11px] text-muted-foreground/80">
            <span className="font-mono tabular-nums">
              {session.actionCount} action{session.actionCount === 1 ? "" : "s"}
            </span>
            <span className="text-muted-foreground/40">•</span>
            <span>{formatRelativeTime(session.startedAt)}</span>
          </div>
          <div className="mt-0.5 truncate font-mono text-[10px] text-muted-foreground/50">
            {session.clientName}
          </div>
        </div>
      </button>
    </li>
  );
}

// ---------------------------------------------------------------------------
// Trace view (selected session)
// ---------------------------------------------------------------------------

function TraceView({
  summary,
  trace,
}: {
  summary: SessionSummary;
  trace: SessionFile;
}) {
  const actions = trace.actions;
  const replay = useReplay(actions);
  const outcomeCounts = useMemo(() => {
    const map = new Map<string, number>();
    for (const a of actions) map.set(a.outcome, (map.get(a.outcome) ?? 0) + 1);
    return map;
  }, [actions]);
  const hasDenials = (outcomeCounts.get("denied") ?? 0) > 0;

  return (
    <>
      <div className="border-b border-border bg-card/30 px-6 py-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-3 min-w-0">
            <Terminal className="size-4 text-muted-foreground" />
            <div className="min-w-0">
              <div className="truncate text-sm font-medium">{summary.serverName}</div>
              <div className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 font-mono text-[10px] text-muted-foreground/70">
                <span>session {summary.id}</span>
                <span>client {summary.clientName}</span>
                <span>sandbox {trace.session.sandbox.mode}</span>
                <span>{formatAbsoluteTime(summary.startedAt)}</span>
              </div>
            </div>
          </div>
          <div className="flex items-center gap-3 text-[11px] font-mono text-muted-foreground">
            <span>{summary.actionCount} actions</span>
            <OutcomeCounters outcomes={outcomeCounts} />
            <span>{formatDurationMs(summary.durationMs)}</span>
            <span
              className={cn(
                "rounded-md border px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider",
                summary.status === "truncated"
                  ? "border-amber-400/40 bg-amber-400/10 text-amber-300"
                  : "border-emerald-500/30 bg-emerald-500/10 text-emerald-300",
              )}
            >
              {summary.status}
            </span>
          </div>
        </div>
      </div>

      {actions.length > 0 && (
        <div className="border-b border-border/60 bg-card/20 px-3 py-2">
          <Scrubber actions={actions} position={replay.position} onSeek={replay.seek} />
          <ReplayControls
            playing={replay.playing}
            speed={replay.speed}
            position={replay.position}
            total={replay.total}
            positionMs={replay.positionMs}
            totalMs={replay.totalMs}
            onToggle={replay.toggle}
            onStepBack={replay.stepBackward}
            onStepForward={replay.stepForward}
            onSetSpeed={replay.setSpeed}
            onJumpToNextDenial={replay.jumpToNextDenial}
            onReset={replay.reset}
            hasDenials={hasDenials}
          />
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto">
        {actions.length === 0 ? (
          <div className="flex h-full items-center justify-center text-xs text-muted-foreground">
            No actions in this session yet.
          </div>
        ) : (
          <ol className="flex flex-col divide-y divide-border/30 px-3 py-2">
            {actions.map((a, i) => (
              <ActionRow
                key={a.id}
                action={a}
                isCurrent={i === replay.position}
                onSelect={() => replay.seek(i)}
              />
            ))}
          </ol>
        )}
      </div>
    </>
  );
}

function OutcomeCounters({ outcomes }: { outcomes: Map<string, number> }) {
  const items = ["ok", "denied", "error"] as const;
  return (
    <span className="flex items-center gap-2">
      {items.map((o) => {
        const count = outcomes.get(o) ?? 0;
        if (count === 0) return null;
        return (
          <span key={o} className="flex items-center gap-1">
            <OutcomeDot outcome={o} />
            <span className="tabular-nums">{count}</span>
          </span>
        );
      })}
    </span>
  );
}

function LoadingTrace() {
  return (
    <div className="flex h-full items-center justify-center text-xs text-muted-foreground">
      Loading trace…
    </div>
  );
}

function EmptyTrace({ onSeed, hasError }: { onSeed: () => void; hasError: boolean }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
      <div className="flex size-12 items-center justify-center rounded-2xl border border-border bg-muted/30">
        <Activity className="size-5 text-muted-foreground" />
      </div>
      <div className="max-w-md space-y-1">
        <h2 className="text-base font-semibold">Open a trace</h2>
        <p className="text-sm text-muted-foreground">
          {hasError
            ? "That session couldn't be loaded — it may be mid-write. The reader handles truncated files; try again in a moment."
            : "Select a session on the left, or seed a sample trace to see what an agent run looks like."}
        </p>
      </div>
      <Button onClick={onSeed} className="mt-1">
        <Sparkles className="size-3.5" /> Seed sample trace
      </Button>
    </div>
  );
}

// ---------------------------------------------------------------------------

function formatRelativeTime(iso: string): string {
  const d = new Date(iso).getTime();
  const diffMs = Date.now() - d;
  if (diffMs < 60_000) return "just now";
  const minutes = Math.floor(diffMs / 60_000);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

function formatAbsoluteTime(iso: string): string {
  const d = new Date(iso);
  const dd = String(d.getDate()).padStart(2, "0");
  const mo = String(d.getMonth() + 1).padStart(2, "0");
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return `${dd}.${mo} ${hh}:${mm}`;
}

function formatDurationMs(ms: number): string {
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)} s`;
  const minutes = Math.floor(ms / 60_000);
  const seconds = Math.floor((ms - minutes * 60_000) / 1000);
  return `${minutes}m ${seconds}s`;
}
