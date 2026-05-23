import { useEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import { ChevronRight } from "lucide-react";

import { cn } from "@/lib/utils";
import type { Action } from "@/types/session";

import { OutcomeBadge, OutcomeDot } from "./outcome-badge";

/**
 * Dense one-line action row, DevTools-style (NOT a CRUD table row). Clicking
 * expands the row inline to show args / result / decision / error details
 * AND seeks the replay engine to this position.
 *
 * The visual hierarchy intentionally puts the outcome dot first and the
 * tool name in monospace — these are the two things you scan for when
 * skimming a long trace.
 *
 * `isCurrent` is the replay playhead. The current row gets a left-accent
 * border + subtle background tint and auto-scrolls itself into view when
 * the replay engine advances. This is the "playback follows trace" UX.
 */
export function ActionRow({
  action,
  isCurrent = false,
  onSelect,
}: {
  action: Action;
  isCurrent?: boolean;
  onSelect?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const rowRef = useRef<HTMLLIElement | null>(null);
  const denied = action.outcome === "denied";
  const errored = action.outcome === "error";

  // Auto-scroll the playhead row into view. `block: "nearest"` avoids
  // jarring jumps when the row is already partially visible.
  useEffect(() => {
    if (isCurrent && rowRef.current) {
      rowRef.current.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
  }, [isCurrent]);

  return (
    <li
      ref={rowRef}
      className={cn(
        "group rounded-md border-l-2 border-transparent transition-colors",
        denied && "bg-destructive/[0.04] hover:bg-destructive/[0.08]",
        errored && "bg-orange-500/[0.03] hover:bg-orange-500/[0.06]",
        !denied && !errored && "hover:bg-card/60",
        isCurrent && "border-l-primary bg-primary/[0.06]",
      )}
    >
      <button
        type="button"
        onClick={() => {
          setOpen((v) => !v);
          onSelect?.();
        }}
        className="flex w-full items-center gap-3 px-3 py-1.5 text-left"
      >
        <ChevronRight
          className={cn(
            "size-3 shrink-0 text-muted-foreground/40 transition-transform",
            open && "rotate-90",
          )}
        />
        <OutcomeDot outcome={action.outcome} className="shrink-0" />
        <span className="w-12 shrink-0 font-mono text-[11px] text-muted-foreground/60 tabular-nums">
          #{action.seq}
        </span>
        <span
          className={cn(
            "min-w-[120px] shrink-0 truncate font-mono text-[13px]",
            denied && "text-destructive line-through decoration-destructive/40",
            errored && "text-orange-300",
            !denied && !errored && "text-foreground",
          )}
        >
          {action.tool ?? action.kind}
        </span>
        <span className="min-w-0 flex-1 truncate font-mono text-[12px] text-muted-foreground/80">
          {renderTargetSummary(action)}
        </span>
        <span className="shrink-0 font-mono text-[11px] tabular-nums text-muted-foreground/70">
          {formatDuration(action.duration_ns)}
        </span>
        <span className="shrink-0 font-mono text-[11px] tabular-nums text-muted-foreground/50">
          {formatTime(action.ts_wall)}
        </span>
      </button>

      {open && (
        <motion.div
          initial={{ opacity: 0, height: 0 }}
          animate={{ opacity: 1, height: "auto" }}
          transition={{ duration: 0.12 }}
          className="overflow-hidden border-t border-border/40 px-3 pb-3 pt-2"
        >
          <div className="flex flex-wrap items-center gap-2 pb-2">
            <OutcomeBadge outcome={action.outcome} />
            {action.payload_truncated && (
              <span className="rounded-md border border-amber-400/30 bg-amber-400/10 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider text-amber-300">
                truncated
              </span>
            )}
            <span className="font-mono text-[10px] text-muted-foreground/60">
              {action.id}
            </span>
          </div>

          {action.error && (
            <DetailBlock label="error" tone="destructive">
              <div className="font-mono text-[11px]">
                <span className="text-destructive/90">{action.error.code}</span>{" "}
                <span className="text-foreground/80">{action.error.message}</span>
                <div className="mt-1 text-muted-foreground/60">
                  source: {action.error.source}
                </div>
              </div>
            </DetailBlock>
          )}

          {action.decision && (
            <DetailBlock label="decision">
              <div className="space-y-0.5 font-mono text-[11px] text-foreground/80">
                <div>
                  <span className="text-muted-foreground/60">verdict</span>{" "}
                  <span
                    className={cn(
                      action.decision.verdict === "deny" && "text-destructive",
                      action.decision.verdict === "allow" && "text-emerald-300",
                    )}
                  >
                    {action.decision.verdict}
                  </span>
                </div>
                <div>
                  <span className="text-muted-foreground/60">rule</span>{" "}
                  <span>{action.decision.rule_id}</span>
                </div>
                <div className="text-foreground/70">{action.decision.reason}</div>
              </div>
            </DetailBlock>
          )}

          {action.args !== null && (
            <DetailBlock label="args">
              <Json value={action.args} />
            </DetailBlock>
          )}
          {action.result !== null && (
            <DetailBlock label="result">
              <Json value={action.result} />
            </DetailBlock>
          )}

          <div className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 font-mono text-[10px] text-muted-foreground/60">
            <span>
              hash: <span className="text-foreground/70">{action.payload_hash}</span>
            </span>
            <span>
              size: <span className="text-foreground/70">{action.payload_size_bytes} B</span>
            </span>
            <span>
              mono_ns: <span className="text-foreground/70">{action.ts_mono_ns}</span>
            </span>
            {action.cause_id && (
              <span>
                cause: <span className="text-foreground/70">{action.cause_id}</span>
              </span>
            )}
          </div>
        </motion.div>
      )}
    </li>
  );
}

// ---------------------------------------------------------------------------

function DetailBlock({
  label,
  children,
  tone,
}: {
  label: string;
  children: React.ReactNode;
  tone?: "destructive";
}) {
  return (
    <div className="mt-2 first:mt-0">
      <div
        className={cn(
          "mb-1 text-[10px] font-medium uppercase tracking-wider",
          tone === "destructive" ? "text-destructive/80" : "text-muted-foreground/60",
        )}
      >
        {label}
      </div>
      <div
        className={cn(
          "rounded-md border bg-[hsl(240_10%_3.5%)] px-2.5 py-2",
          tone === "destructive"
            ? "border-destructive/30"
            : "border-border/50",
        )}
      >
        {children}
      </div>
    </div>
  );
}

function Json({ value }: { value: unknown }) {
  return (
    <pre className="overflow-x-auto whitespace-pre-wrap font-mono text-[11px] leading-relaxed text-foreground/80">
      {safeStringify(value)}
    </pre>
  );
}

function safeStringify(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function renderTargetSummary(action: Action): string {
  // The first identifying string we can find. Order matters — `path` is
  // most common, then `url`, then a generic stringified-value fallback.
  if (action.args && typeof action.args === "object") {
    const a = action.args as Record<string, unknown>;
    for (const key of ["path", "url", "command", "query", "name"]) {
      const v = a[key];
      if (typeof v === "string" && v.length > 0) return v;
    }
    const event = a["event"];
    if (typeof event === "string") return `event: ${event}`;
  }
  return action.tool ? "" : action.kind;
}

function formatDuration(durationNs: number | null): string {
  if (durationNs === null) return "—";
  const ms = durationNs / 1_000_000;
  if (ms < 1) return `${(durationNs / 1_000).toFixed(0)} μs`;
  if (ms < 1_000) return `${ms.toFixed(1)} ms`;
  return `${(ms / 1_000).toFixed(2)} s`;
}

function formatTime(iso: string): string {
  const d = new Date(iso);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  const mss = String(d.getMilliseconds()).padStart(3, "0");
  return `${hh}:${mm}:${ss}.${mss}`;
}
