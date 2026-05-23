import { Search } from "lucide-react";

import { Input } from "@/components/ui/input";

/**
 * Titlebar shown at the top of the window. Doubles as the draggable region
 * (via `data-tauri-drag-region`) because we hide the native title bar in
 * `tauri.conf.json` for a more app-like feel (think Linear / Raycast).
 */
export function Titlebar() {
  return (
    <header
      data-tauri-drag-region
      className="flex h-11 shrink-0 items-center gap-3 border-b border-border bg-background/80 px-4 backdrop-blur-md"
    >
      <div className="relative max-w-md flex-1">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input
          placeholder="Search servers, marketplace, logs…"
          className="h-7 border-none bg-muted/60 pl-8 text-xs focus-visible:ring-1"
        />
      </div>

      <div className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
        <span className="hidden sm:inline">v0.1.0</span>
      </div>
    </header>
  );
}
