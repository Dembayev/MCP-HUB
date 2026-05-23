import { motion } from "framer-motion";

import { cn } from "@/lib/utils";
import type { Category } from "@/types/marketplace";

export type CategoryFilter = "all" | Category;

const ORDER: { key: CategoryFilter; label: string }[] = [
  { key: "all", label: "All" },
  { key: "filesystem", label: "Filesystem" },
  { key: "developer-tools", label: "Dev tools" },
  { key: "web", label: "Web" },
  { key: "data", label: "Data" },
  { key: "productivity", label: "Productivity" },
  { key: "ai", label: "AI" },
  { key: "other", label: "Other" },
];

interface CategoryChipsProps {
  current: CategoryFilter;
  onChange: (next: CategoryFilter) => void;
  /** Map of category -> count of entries in that category, for the badge. */
  counts?: Partial<Record<CategoryFilter, number>>;
}

export function CategoryChips({ current, onChange, counts }: CategoryChipsProps) {
  return (
    <div className="flex flex-wrap items-center gap-1">
      {ORDER.map((c) => {
        const active = current === c.key;
        const count = counts?.[c.key];
        return (
          <button
            key={c.key}
            type="button"
            onClick={() => onChange(c.key)}
            className={cn(
              "relative flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-medium transition-colors",
              active
                ? "text-foreground"
                : "text-muted-foreground hover:bg-accent/40 hover:text-foreground",
            )}
          >
            {active && (
              <motion.div
                layoutId="category-active"
                className="absolute inset-0 -z-10 rounded-md bg-accent"
                transition={{ type: "spring", stiffness: 600, damping: 40 }}
              />
            )}
            <span>{c.label}</span>
            {typeof count === "number" && count > 0 && (
              <span
                className={cn(
                  "rounded px-1 text-[10px]",
                  active
                    ? "bg-foreground/10 text-foreground"
                    : "text-muted-foreground/70",
                )}
              >
                {count}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
