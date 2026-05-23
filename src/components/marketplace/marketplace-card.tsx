import { motion } from "framer-motion";
import { Check, ExternalLink, Star } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { cn } from "@/lib/utils";
import type { MarketplaceEntry } from "@/types/marketplace";

import { PermissionPills } from "./permission-list";
import { TrustBadge } from "./trust-badge";

interface MarketplaceCardProps {
  entry: MarketplaceEntry;
  installed: boolean;
  onInstall: (entry: MarketplaceEntry) => void;
}

export function MarketplaceCard({
  entry,
  installed,
  onInstall,
}: MarketplaceCardProps) {
  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.18, ease: "easeOut" }}
    >
      <Card className="group flex h-full flex-col p-5 transition-colors hover:border-border/80 hover:bg-card/80">
        <div className="flex items-start gap-3">
          <div
            className={cn(
              "flex size-11 shrink-0 items-center justify-center rounded-lg border text-sm font-semibold tracking-tight",
              "border-border bg-gradient-to-br from-muted/40 to-muted/10",
            )}
            aria-hidden
          >
            {entry.name.slice(0, 2).toUpperCase()}
          </div>

          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <h3 className="truncate font-medium tracking-tight">{entry.name}</h3>
              <TrustBadge trust={entry.trust} />
            </div>
            <p className="mt-0.5 text-xs text-muted-foreground">
              by {entry.author} · v{entry.version}
            </p>
          </div>

          <a
            href={entry.homepage}
            target="_blank"
            rel="noreferrer"
            onClick={(e) => e.stopPropagation()}
            className="rounded-md p-1.5 text-muted-foreground opacity-0 transition-all hover:bg-accent group-hover:opacity-100"
            aria-label="Open homepage"
          >
            <ExternalLink className="size-3.5" />
          </a>
        </div>

        <p className="mt-3 line-clamp-2 text-sm text-muted-foreground">
          {entry.description}
        </p>

        <div className="mt-4 flex items-center justify-between text-xs text-muted-foreground">
          <span className="inline-flex items-center gap-1">
            <Star className="size-3 fill-muted-foreground/40" />
            {entry.stars.toLocaleString()}
          </span>
          <span>{entry.installSize}</span>
        </div>

        <div className="mt-3 flex items-center gap-2 border-t border-border/60 pt-3">
          <PermissionPills permissions={entry.permissions} />
          <div className="ml-auto">
            {installed ? (
              <Button size="sm" variant="secondary" disabled>
                <Check className="size-3.5" />
                Installed
              </Button>
            ) : (
              <Button size="sm" onClick={() => onInstall(entry)}>
                Install
              </Button>
            )}
          </div>
        </div>
      </Card>
    </motion.div>
  );
}
