import { useCallback, useRef } from "react";

import { cn } from "@/lib/utils";
import type { Action, ActionOutcome } from "@/types/session";

/**
 * Horizontal scrubber strip — one tick per action, colored by outcome, with
 * a playhead overlay at the current position. Clicking or dragging seeks.
 *
 * Visual contract (per launch_guardrails): preserve DevTools / Chrome-trace
 * aesthetic — dense ticks, not a video-player slider. The denial moment
 * (red) is the eye-catching element and the scrubber must NOT dilute it.
 */
export function Scrubber({
  actions,
  position,
  onSeek,
}: {
  actions: Action[];
  position: number;
  onSeek: (index: number) => void;
}) {
  const trackRef = useRef<HTMLDivElement | null>(null);
  const draggingRef = useRef(false);

  const seekFromPointer = useCallback(
    (clientX: number) => {
      const el = trackRef.current;
      if (!el || actions.length === 0) return;
      const rect = el.getBoundingClientRect();
      const pct = (clientX - rect.left) / rect.width;
      const idx = Math.round(pct * (actions.length - 1));
      onSeek(clamp(idx, 0, actions.length - 1));
    },
    [actions.length, onSeek],
  );

  if (actions.length === 0) {
    return (
      <div className="h-8 rounded-md border border-dashed border-border/40 bg-card/30" />
    );
  }

  const positionPct = (position / Math.max(1, actions.length - 1)) * 100;

  return (
    <div
      ref={trackRef}
      role="slider"
      aria-valuemin={0}
      aria-valuemax={actions.length - 1}
      aria-valuenow={position}
      tabIndex={0}
      className={cn(
        "relative h-8 select-none rounded-md border border-border bg-card/40 px-1",
        "cursor-crosshair hover:border-border/80",
      )}
      onPointerDown={(e) => {
        e.preventDefault();
        draggingRef.current = true;
        trackRef.current?.setPointerCapture(e.pointerId);
        seekFromPointer(e.clientX);
      }}
      onPointerMove={(e) => {
        if (!draggingRef.current) return;
        seekFromPointer(e.clientX);
      }}
      onPointerUp={(e) => {
        draggingRef.current = false;
        trackRef.current?.releasePointerCapture(e.pointerId);
      }}
      onKeyDown={(e) => {
        if (e.key === "ArrowLeft") {
          e.preventDefault();
          onSeek(Math.max(0, position - 1));
        } else if (e.key === "ArrowRight") {
          e.preventDefault();
          onSeek(Math.min(actions.length - 1, position + 1));
        } else if (e.key === "Home") {
          e.preventDefault();
          onSeek(0);
        } else if (e.key === "End") {
          e.preventDefault();
          onSeek(actions.length - 1);
        }
      }}
    >
      <Ticks actions={actions} />
      <Playhead pct={positionPct} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Ticks — one bar per action, colored by outcome.
// ---------------------------------------------------------------------------

function Ticks({ actions }: { actions: Action[] }) {
  // For large traces we'd cluster, but at MVP scale (10s–100s of actions)
  // each tick gets its own slot. Width is automatically pct-based so the
  // strip scales with container width.
  return (
    <div className="absolute inset-x-1 inset-y-1.5 flex gap-px">
      {actions.map((a) => (
        <span
          key={a.id}
          title={`#${a.seq} ${a.tool ?? a.kind} — ${a.outcome}`}
          className={cn("flex-1 rounded-[1px]", outcomeTickClass(a.outcome))}
        />
      ))}
    </div>
  );
}

function outcomeTickClass(outcome: ActionOutcome): string {
  switch (outcome) {
    case "ok":
      return "bg-emerald-500/50";
    case "denied":
      // Denials are the demo climax — let them visually dominate the strip.
      return "bg-destructive shadow-[0_0_6px_rgba(248,113,113,0.45)]";
    case "error":
      return "bg-orange-500/70";
    case "timeout":
      return "bg-amber-400/60";
    case "cancelled":
      return "bg-muted-foreground/40";
    default:
      return "bg-muted-foreground/30";
  }
}

// ---------------------------------------------------------------------------
// Playhead — vertical line overlay at the current position.
// ---------------------------------------------------------------------------

function Playhead({ pct }: { pct: number }) {
  return (
    <span
      aria-hidden
      className="pointer-events-none absolute inset-y-0 w-px bg-foreground/90 shadow-[0_0_6px_rgba(255,255,255,0.4)] transition-[left] duration-100"
      style={{ left: `calc(${pct}% + 0.25rem)` }}
    >
      {/* Small handle dot for visibility */}
      <span className="absolute left-1/2 top-1/2 size-2 -translate-x-1/2 -translate-y-1/2 rounded-full bg-foreground" />
    </span>
  );
}

function clamp(value: number, lo: number, hi: number): number {
  if (value < lo) return lo;
  if (value > hi) return hi;
  return value;
}
