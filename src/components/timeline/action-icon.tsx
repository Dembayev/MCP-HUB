import {
  Brain,
  Code2,
  FileEdit,
  FileText,
  Globe,
  MousePointerClick,
  Search,
  Terminal,
  Wrench,
  type LucideIcon,
} from "lucide-react";

import { cn } from "@/lib/utils";
import type { ActionKind } from "@/types/actions";

interface ActionIconMeta {
  icon: LucideIcon;
  label: string;
  /** Tailwind text + bg pair, on a tinted muted ring. */
  tone: string;
}

export const ACTION_META: Record<ActionKind, ActionIconMeta> = {
  "fs-read": {
    icon: FileText,
    label: "Read",
    tone: "text-blue-400 bg-blue-500/10 ring-blue-500/20",
  },
  "fs-write": {
    icon: FileEdit,
    label: "Write",
    tone: "text-sky-300 bg-sky-500/10 ring-sky-500/20",
  },
  "browser-open": {
    icon: MousePointerClick,
    label: "Browser",
    tone: "text-cyan-300 bg-cyan-500/10 ring-cyan-500/20",
  },
  "http-fetch": {
    icon: Globe,
    label: "Fetch",
    tone: "text-emerald-300 bg-emerald-500/10 ring-emerald-500/20",
  },
  "terminal-exec": {
    icon: Terminal,
    label: "Shell",
    tone: "text-orange-300 bg-orange-500/10 ring-orange-500/20",
  },
  "memory-store": {
    icon: Brain,
    label: "Memory",
    tone: "text-purple-300 bg-purple-500/10 ring-purple-500/20",
  },
  search: {
    icon: Search,
    label: "Search",
    tone: "text-yellow-300 bg-yellow-500/10 ring-yellow-500/20",
  },
  "tool-call": {
    icon: Wrench,
    label: "Tool",
    tone: "text-muted-foreground bg-muted/40 ring-border",
  },
  other: {
    icon: Code2,
    label: "Other",
    tone: "text-muted-foreground bg-muted/40 ring-border",
  },
};

interface ActionIconProps {
  kind: ActionKind;
  className?: string;
}

export function ActionIcon({ kind, className }: ActionIconProps) {
  const meta = ACTION_META[kind];
  const Icon = meta.icon;
  return (
    <span
      className={cn(
        "flex size-8 items-center justify-center rounded-lg ring-1",
        meta.tone,
        className,
      )}
    >
      <Icon className="size-4" />
    </span>
  );
}
