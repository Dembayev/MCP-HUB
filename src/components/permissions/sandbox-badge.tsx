import { ShieldCheck, ShieldOff } from "lucide-react";

import { cn } from "@/lib/utils";

interface SandboxBadgeProps {
  enforcement: string;
  className?: string;
}

/**
 * Pill showing whether the OS-level sandbox is actually enforcing what's
 * configured. Currently macOS = enforced, everything else = best-effort.
 * Honest about the limitation.
 */
export function SandboxBadge({ enforcement, className }: SandboxBadgeProps) {
  const enforced = enforcement === "macos-sandbox-exec";
  const Icon = enforced ? ShieldCheck : ShieldOff;
  return (
    <span
      title={
        enforced
          ? "Enforced via macOS sandbox-exec. Granted scopes are applied to spawned MCP servers."
          : "Sandbox enforcement is not yet wired on this platform — permissions are recorded but not applied at the OS level."
      }
      className={cn(
        "inline-flex items-center gap-1.5 rounded-md border px-2 py-0.5 text-[11px] font-medium",
        enforced
          ? "border-success/40 bg-success/10 text-success"
          : "border-yellow-500/40 bg-yellow-500/10 text-yellow-400",
        className,
      )}
    >
      <Icon className="size-3" />
      {enforced ? "Sandbox enforced" : "Sandbox: best-effort"}
    </span>
  );
}
