import { CheckCircle2, CircleAlert, Clock, ShieldX, X } from "lucide-react";

import { cn } from "@/lib/utils";
import type { ActionOutcome } from "@/types/session";

/**
 * Color-coded outcome glyph for a single Action — the visual primitive that
 * makes denial moments (red) "pop" in a long timeline. Used in dense rows;
 * the detail panel uses the same colour mapping for badges.
 */
export function OutcomeDot({
  outcome,
  className,
}: {
  outcome: ActionOutcome;
  className?: string;
}) {
  return (
    <span
      aria-label={outcome}
      className={cn(
        "inline-block size-2 rounded-full",
        outcome === "ok" && "bg-emerald-500/80",
        outcome === "denied" && "bg-destructive shadow-[0_0_10px_rgba(248,113,113,0.6)]",
        outcome === "error" && "bg-orange-500/90",
        outcome === "timeout" && "bg-amber-400/80",
        outcome === "cancelled" && "bg-muted-foreground/60",
        outcome === "unknown" && "bg-muted-foreground/40",
        className,
      )}
    />
  );
}

export function OutcomeBadge({ outcome }: { outcome: ActionOutcome }) {
  const meta = META[outcome] ?? META.unknown;
  const Icon = meta.Icon;
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider",
        meta.classes,
      )}
    >
      <Icon className="size-3" /> {meta.label}
    </span>
  );
}

const META: Record<
  ActionOutcome,
  { label: string; Icon: typeof CheckCircle2; classes: string }
> = {
  ok: {
    label: "ok",
    Icon: CheckCircle2,
    classes: "border-emerald-500/30 bg-emerald-500/10 text-emerald-300",
  },
  denied: {
    label: "denied",
    Icon: ShieldX,
    classes: "border-destructive/40 bg-destructive/15 text-destructive",
  },
  error: {
    label: "error",
    Icon: CircleAlert,
    classes: "border-orange-500/40 bg-orange-500/15 text-orange-300",
  },
  timeout: {
    label: "timeout",
    Icon: Clock,
    classes: "border-amber-400/40 bg-amber-400/15 text-amber-300",
  },
  cancelled: {
    label: "cancelled",
    Icon: X,
    classes: "border-muted-foreground/30 bg-muted/30 text-muted-foreground",
  },
  unknown: {
    label: "unknown",
    Icon: CircleAlert,
    classes: "border-muted-foreground/20 bg-muted/20 text-muted-foreground/70",
  },
};
