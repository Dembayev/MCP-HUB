import { cn } from "@/lib/utils";
import type { ServerStatus } from "@/types/mcp";

interface StatusDotProps {
  status: ServerStatus;
  className?: string;
}

const COLORS: Record<ServerStatus, string> = {
  running: "bg-success",
  starting: "bg-yellow-400 animate-pulse-soft",
  stopped: "bg-muted-foreground/50",
  crashed: "bg-destructive",
};

export function StatusDot({ status, className }: StatusDotProps) {
  return (
    <span className={cn("relative inline-flex size-2", className)}>
      <span
        className={cn(
          "absolute inset-0 rounded-full opacity-40 blur-[2px]",
          COLORS[status],
        )}
      />
      <span className={cn("relative size-2 rounded-full", COLORS[status])} />
    </span>
  );
}
