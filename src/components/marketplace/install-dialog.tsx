import { useState } from "react";
import { Loader2, ShieldCheck } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import type { MarketplaceEntry } from "@/types/marketplace";

import { PermissionList } from "./permission-list";
import { TrustBadge } from "./trust-badge";

interface InstallDialogProps {
  entry: MarketplaceEntry | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: (entry: MarketplaceEntry) => Promise<void>;
}

/**
 * The trust-surface modal. Renders the Android-style permission preview
 * before the user actually installs an MCP server. Two buttons — Allow once
 * (install) and Cancel. "Always allow" / "Allow once" granularity will come
 * with the real sandbox.
 */
export function InstallDialog({
  entry,
  open,
  onOpenChange,
  onConfirm,
}: InstallDialogProps) {
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!entry) return null;

  const unsafe = entry.trust === "unsafe";
  const requiresEnv = (entry.requiredEnv?.length ?? 0) > 0;

  const handleConfirm = async () => {
    setError(null);
    setInstalling(true);
    try {
      await onConfirm(entry);
      onOpenChange(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setInstalling(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <div className="flex items-center gap-3">
            <div className="flex size-10 items-center justify-center rounded-lg border border-border bg-muted/40 text-sm font-semibold">
              {entry.name.slice(0, 2).toUpperCase()}
            </div>
            <div>
              <DialogTitle>Install {entry.name}?</DialogTitle>
              <DialogDescription>
                by {entry.author} · v{entry.version} · {entry.installSize}
              </DialogDescription>
            </div>
          </div>
        </DialogHeader>

        <div className="space-y-4">
          <TrustBadge trust={entry.trust} variant="row" />

          <div>
            <div className="mb-2 flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
              <ShieldCheck className="size-3.5" />
              This server is asking for
            </div>
            <PermissionList permissions={entry.permissions} />
          </div>

          {requiresEnv && (
            <div className="rounded-lg border border-yellow-500/30 bg-yellow-500/5 p-3 text-xs">
              <div className="font-medium text-yellow-400">Configuration required</div>
              <p className="mt-1 text-muted-foreground">
                You'll need to provide the following environment variables before
                the server can run:{" "}
                <span className="font-mono">
                  {entry.requiredEnv?.join(", ")}
                </span>
              </p>
            </div>
          )}

          {unsafe && (
            <div className="rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive">
              <strong className="font-medium">Heads up.</strong> This server can
              do anything your shell can — including delete files and exfiltrate
              data. Only install if you trust the source.
            </div>
          )}

          {error && (
            <div className="rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive">
              {error}
            </div>
          )}
        </div>

        <DialogFooter className="gap-2">
          <Button
            variant="ghost"
            onClick={() => onOpenChange(false)}
            disabled={installing}
          >
            Cancel
          </Button>
          <Button
            onClick={() => void handleConfirm()}
            disabled={installing}
            className={cn(unsafe && "bg-destructive hover:bg-destructive/90")}
          >
            {installing ? (
              <>
                <Loader2 className="size-3.5 animate-spin" />
                Installing…
              </>
            ) : (
              <>{unsafe ? "Install anyway" : "Allow & Install"}</>
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
