import { Boxes, Plus } from "lucide-react";

import { Button } from "@/components/ui/button";

interface EmptyStateProps {
  onInstall?: () => void;
}

export function EmptyState({ onInstall }: EmptyStateProps) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 py-20 text-center">
      <div className="flex size-14 items-center justify-center rounded-2xl border border-border bg-muted/30">
        <Boxes className="size-6 text-muted-foreground" />
      </div>
      <div className="space-y-1">
        <h2 className="text-lg font-semibold tracking-tight">No servers yet</h2>
        <p className="max-w-sm text-sm text-muted-foreground">
          Install an MCP server from the marketplace, or add one manually by
          pointing to a command.
        </p>
      </div>
      <Button onClick={onInstall} className="mt-2">
        <Plus className="size-3.5" />
        Install a server
      </Button>
    </div>
  );
}
