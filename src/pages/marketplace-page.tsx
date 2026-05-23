import { useMemo, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { Search, Sparkles } from "lucide-react";

import { CategoryChips, type CategoryFilter } from "@/components/marketplace/category-chips";
import { InstallDialog } from "@/components/marketplace/install-dialog";
import { MarketplaceCard } from "@/components/marketplace/marketplace-card";
import { Input } from "@/components/ui/input";
import { MARKETPLACE } from "@/data/marketplace";
import { useMcpServers } from "@/hooks/use-mcp-servers";
import type { MarketplaceEntry } from "@/types/marketplace";

export function MarketplacePage() {
  const { install, installedMarketplaceIds } = useMcpServers();

  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<CategoryFilter>("all");
  const [selected, setSelected] = useState<MarketplaceEntry | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return MARKETPLACE.filter((entry) => {
      if (category !== "all" && entry.category !== category) return false;
      if (!q) return true;
      return (
        entry.name.toLowerCase().includes(q) ||
        entry.description.toLowerCase().includes(q) ||
        entry.author.toLowerCase().includes(q)
      );
    });
  }, [query, category]);

  const counts = useMemo(() => {
    const c: Partial<Record<CategoryFilter, number>> = { all: MARKETPLACE.length };
    for (const e of MARKETPLACE) {
      c[e.category] = (c[e.category] ?? 0) + 1;
    }
    return c;
  }, []);

  const openInstall = (entry: MarketplaceEntry) => {
    setSelected(entry);
    setDialogOpen(true);
  };

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="flex flex-col gap-3 border-b border-border px-6 py-4">
        <div className="flex items-center justify-between gap-4">
          <div>
            <h1 className="text-xl font-semibold tracking-tight">Marketplace</h1>
            <p className="text-xs text-muted-foreground">
              {MARKETPLACE.length} servers · curated, with explicit permissions
            </p>
          </div>

          <div className="relative w-full max-w-xs">
            <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search the marketplace…"
              className="h-8 pl-8 text-xs"
            />
          </div>
        </div>

        <CategoryChips current={category} onChange={setCategory} counts={counts} />
      </div>

      {/* Grid */}
      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-6 animate-fade-in">
        {filtered.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
            <div className="flex size-12 items-center justify-center rounded-2xl border border-border bg-muted/30">
              <Sparkles className="size-5 text-muted-foreground" />
            </div>
            <div>
              <h2 className="text-base font-semibold">No matches</h2>
              <p className="mt-1 text-sm text-muted-foreground">
                Try a different keyword or category.
              </p>
            </div>
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
            <AnimatePresence mode="popLayout">
              {filtered.map((entry) => (
                <MarketplaceCard
                  key={entry.id}
                  entry={entry}
                  installed={installedMarketplaceIds.has(entry.id)}
                  onInstall={openInstall}
                />
              ))}
            </AnimatePresence>
          </div>
        )}
      </div>

      <InstallDialog
        entry={selected}
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        onConfirm={async (entry) => {
          await install(entry);
        }}
      />
    </div>
  );
}
