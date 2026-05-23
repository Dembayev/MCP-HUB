import {
  ClipboardCopy,
  FileText,
  FilePlus,
  Globe,
  KeyRound,
  Mic,
  PlayCircle,
  Terminal,
  MousePointerClick,
  type LucideIcon,
} from "lucide-react";

import { cn } from "@/lib/utils";
import type { PermissionScope, RequiredPermission } from "@/types/marketplace";

const SCOPE_META: Record<
  PermissionScope,
  { label: string; icon: LucideIcon; tone: string }
> = {
  "fs.read":     { label: "Read files",        icon: FileText,           tone: "text-blue-400" },
  "fs.write":    { label: "Write files",       icon: FilePlus,           tone: "text-blue-300" },
  internet:      { label: "Internet",          icon: Globe,              tone: "text-emerald-400" },
  browser:       { label: "Browser",           icon: MousePointerClick,  tone: "text-cyan-400" },
  terminal:      { label: "Shell",             icon: Terminal,           tone: "text-destructive" },
  clipboard:     { label: "Clipboard",         icon: ClipboardCopy,      tone: "text-muted-foreground" },
  microphone:    { label: "Microphone",        icon: Mic,                tone: "text-destructive" },
  "env.read":    { label: "Environment vars",  icon: KeyRound,           tone: "text-yellow-400" },
  exec:          { label: "Spawn processes",   icon: PlayCircle,         tone: "text-orange-400" },
};

interface PermissionPillsProps {
  permissions: RequiredPermission[];
  className?: string;
  max?: number;
}

/** Compact icon-only pills for cards. */
export function PermissionPills({
  permissions,
  className,
  max = 4,
}: PermissionPillsProps) {
  if (permissions.length === 0) {
    return (
      <span className={cn("text-xs text-muted-foreground", className)}>
        No special permissions
      </span>
    );
  }

  const visible = permissions.slice(0, max);
  const overflow = permissions.length - visible.length;

  return (
    <div className={cn("flex items-center gap-1", className)}>
      {visible.map((p, i) => {
        const meta = SCOPE_META[p.scope];
        const Icon = meta.icon;
        return (
          <span
            key={`${p.scope}-${i}`}
            title={`${meta.label}${p.target ? ` — ${p.target}` : ""}`}
            className="flex size-6 items-center justify-center rounded-md border border-border bg-muted/30"
          >
            <Icon className={cn("size-3", meta.tone)} />
          </span>
        );
      })}
      {overflow > 0 && (
        <span className="text-xs text-muted-foreground">+{overflow}</span>
      )}
    </div>
  );
}

interface PermissionListProps {
  permissions: RequiredPermission[];
  className?: string;
}

/** Detailed list shown inside the install dialog. */
export function PermissionList({ permissions, className }: PermissionListProps) {
  if (permissions.length === 0) {
    return (
      <p className={cn("text-sm text-muted-foreground", className)}>
        This server doesn't request any system permissions.
      </p>
    );
  }

  return (
    <ul className={cn("divide-y divide-border rounded-lg border border-border", className)}>
      {permissions.map((p, i) => {
        const meta = SCOPE_META[p.scope];
        const Icon = meta.icon;
        return (
          <li key={`${p.scope}-${i}`} className="flex items-start gap-3 p-3">
            <div className="flex size-7 shrink-0 items-center justify-center rounded-md border border-border bg-muted/30">
              <Icon className={cn("size-3.5", meta.tone)} />
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex items-baseline gap-2">
                <span className="text-sm font-medium">{meta.label}</span>
                {p.target && (
                  <span className="truncate font-mono text-[11px] text-muted-foreground">
                    {p.target}
                  </span>
                )}
              </div>
              <p className="mt-0.5 text-xs text-muted-foreground">{p.reason}</p>
            </div>
          </li>
        );
      })}
    </ul>
  );
}
