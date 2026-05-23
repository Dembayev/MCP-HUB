import { useState } from "react";
import { motion } from "framer-motion";
import { AlertTriangle, ChevronDown, Loader2, ShieldX } from "lucide-react";

import { cn } from "@/lib/utils";
import type { AgentAction } from "@/types/actions";

import { ACTION_META, ActionIcon } from "./action-icon";

interface TimelineCardProps {
  action: AgentAction;
  /** Show the connector line above the card. Hide for the first item. */
  showConnector: boolean;
}

export function TimelineCard({ action, showConnector }: TimelineCardProps) {
  const [expanded, setExpanded] = useState(false);
  const meta = ACTION_META[action.kind];
  const pending = action.status === "pending";
  const denied = action.status === "denied";
  const failed = action.status === "failed";
  const cancelled = action.status === "cancelled";

  return (
    <motion.li
      layout
      initial={{ opacity: 0, y: 12, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
      className="relative flex gap-3"
    >
      {/* Rail — icon + vertical connector. */}
      <div className="relative flex shrink-0 flex-col items-center">
        {showConnector && (
          <span
            aria-hidden
            className="absolute -top-3 h-3 w-px bg-border"
          />
        )}
        <div className="relative">
          <ActionIcon
            kind={action.kind}
            className={cn(
              denied && "opacity-70 ring-destructive/50",
              failed && "opacity-80 ring-orange-500/40",
              pending && "ring-primary/60",
            )}
          />
          {pending && (
            <span
              aria-hidden
              className="pointer-events-none absolute inset-0 rounded-lg ring-2 ring-primary/40 animate-pulse-soft"
            />
          )}
        </div>
        <span
          aria-hidden
          className="mt-1 w-px flex-1 bg-border"
        />
      </div>

      {/* Card body */}
      <div
        className={cn(
          "mb-3 min-w-0 flex-1 rounded-xl border transition-colors",
          pending && "border-primary/30 bg-primary/[0.04]",
          denied && "border-destructive/40 bg-destructive/[0.04] hover:border-destructive/60",
          failed && "border-orange-500/40 bg-orange-500/[0.04]",
          cancelled && "border-border/50 bg-card/40 opacity-70",
          !pending && !denied && !failed && !cancelled &&
            "border-border bg-card/60 hover:border-border/80 hover:bg-card/80",
        )}
      >
        <button
          type="button"
          onClick={() => setExpanded((v) => !v)}
          className="flex w-full items-center gap-3 p-3 text-left"
        >
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span className="text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
                {meta.label}
              </span>
              <span className="font-mono text-[11px] text-muted-foreground/60">
                {action.toolName}
              </span>
              {pending && (
                <span className="inline-flex items-center gap-1 rounded-md border border-primary/30 bg-primary/10 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider text-primary">
                  <Loader2 className="size-3 animate-spin" /> Pending
                </span>
              )}
              {denied && (
                <span className="inline-flex items-center gap-1 rounded-md border border-destructive/40 bg-destructive/15 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider text-destructive">
                  <ShieldX className="size-3" /> Denied
                </span>
              )}
              {failed && (
                <span className="inline-flex items-center gap-1 rounded-md border border-orange-500/40 bg-orange-500/15 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider text-orange-400">
                  <AlertTriangle className="size-3" /> Failed
                </span>
              )}
            </div>
            <div
              className={cn(
                "mt-0.5 truncate font-mono text-sm",
                denied && "text-destructive/90 line-through decoration-destructive/50",
                failed && "text-orange-200",
                !denied && !failed && (action.target ? "text-foreground" : "italic text-muted-foreground"),
              )}
            >
              {action.target ?? "(no target)"}
            </div>
            {denied && action.deniedReason && (
              <div className="mt-1 text-[11px] text-destructive/80">
                {action.deniedReason}
              </div>
            )}
            {failed && action.error && (
              <div className="mt-1 text-[11px] text-orange-300/80">
                {action.error}
              </div>
            )}
          </div>

          <div className="flex shrink-0 items-center gap-2">
            {action.latencyMs !== null && (
              <span
                className={cn(
                  "rounded-md border px-1.5 py-0.5 font-mono text-[10px]",
                  denied
                    ? "border-destructive/30 text-destructive/80"
                    : failed
                      ? "border-orange-500/30 text-orange-300/90"
                      : "border-border bg-muted/30 text-muted-foreground",
                )}
              >
                {formatLatency(action.latencyMs)}
              </span>
            )}
            <span className="font-mono text-[11px] text-muted-foreground/70">
              {pending ? "…" : formatTime(action.timestamp)}
            </span>
            {action.params && (
              <ChevronDown
                className={cn(
                  "size-3.5 text-muted-foreground transition-transform",
                  expanded && "rotate-180",
                )}
              />
            )}
          </div>
        </button>

        {expanded && action.params && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            transition={{ duration: 0.15 }}
            className="border-t border-border/60 px-3 pb-3"
          >
            <pre className="mt-2 overflow-x-auto rounded-md border border-border/40 bg-[hsl(240_10%_3%)] p-2.5 font-mono text-[11px] leading-relaxed text-foreground/80">
              {JSON.stringify(action.params, null, 2)}
            </pre>
            {action.requestId !== null && (
              <div className="mt-2 text-[10px] text-muted-foreground/60">
                request id: {String(action.requestId)}
              </div>
            )}
          </motion.div>
        )}
      </div>
    </motion.li>
  );
}

function formatLatency(ms: number): string {
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(2)} s`;
  return `${Math.round(ms / 1000)} s`;
}

function formatTime(iso: string): string {
  const d = new Date(iso);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  return `${hh}:${mm}:${ss}`;
}
