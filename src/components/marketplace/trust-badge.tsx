import {
  AlertTriangle,
  BadgeCheck,
  FlaskConical,
  ShieldAlert,
  type LucideIcon,
} from "lucide-react";

import { cn } from "@/lib/utils";
import type { TrustBadge as TrustBadgeKind } from "@/types/marketplace";

interface TrustBadgeProps {
  trust: TrustBadgeKind;
  className?: string;
  /** Compact pill (icon + short label) or full row with tooltip-style copy. */
  variant?: "pill" | "row";
}

interface Meta {
  label: string;
  description: string;
  icon: LucideIcon;
  classes: string;
}

const META: Record<TrustBadgeKind, Meta> = {
  verified: {
    label: "Verified",
    description: "Reviewed by the MCP Hub team.",
    icon: BadgeCheck,
    classes: "bg-success/15 text-success border-success/30",
  },
  "community-trusted": {
    label: "Community Trusted",
    description: "Widely used and vetted by the community.",
    icon: ShieldAlert,
    classes: "bg-primary/15 text-primary border-primary/30",
  },
  experimental: {
    label: "Experimental",
    description: "Useful but young — expect rough edges.",
    icon: FlaskConical,
    classes: "bg-yellow-500/15 text-yellow-400 border-yellow-500/30",
  },
  unsafe: {
    label: "Unsafe",
    description: "Known security risks. Review before installing.",
    icon: AlertTriangle,
    classes: "bg-destructive/15 text-destructive border-destructive/30",
  },
};

export function TrustBadge({ trust, className, variant = "pill" }: TrustBadgeProps) {
  const meta = META[trust];
  const Icon = meta.icon;

  if (variant === "row") {
    return (
      <div
        className={cn(
          "flex items-center gap-2.5 rounded-md border px-3 py-2 text-xs",
          meta.classes,
          className,
        )}
      >
        <Icon className="size-4 shrink-0" />
        <div className="min-w-0">
          <div className="font-medium leading-tight">{meta.label}</div>
          <div className="text-[11px] opacity-80">{meta.description}</div>
        </div>
      </div>
    );
  }

  return (
    <span
      title={meta.description}
      className={cn(
        "inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 text-[11px] font-medium",
        meta.classes,
        className,
      )}
    >
      <Icon className="size-3" />
      {meta.label}
    </span>
  );
}
