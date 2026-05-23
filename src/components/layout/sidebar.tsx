import {
  Activity,
  Boxes,
  GitBranch,
  ShieldCheck,
  Settings,
  Store,
  type LucideIcon,
} from "lucide-react";
import { motion } from "framer-motion";

import { cn } from "@/lib/utils";

export type Route =
  | "servers"
  | "marketplace"
  | "timeline"
  | "activity"
  | "permissions"
  | "settings";

interface NavItem {
  route: Route;
  label: string;
  icon: LucideIcon;
}

const NAV: NavItem[] = [
  { route: "servers", label: "Servers", icon: Boxes },
  { route: "marketplace", label: "Marketplace", icon: Store },
  { route: "timeline", label: "Timeline", icon: GitBranch },
  { route: "activity", label: "Activity", icon: Activity },
  { route: "permissions", label: "Permissions", icon: ShieldCheck },
  { route: "settings", label: "Settings", icon: Settings },
];

interface SidebarProps {
  current: Route;
  onSelect: (route: Route) => void;
}

export function Sidebar({ current, onSelect }: SidebarProps) {
  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-border bg-card/40">
      {/*
        Sidebar header doubles as the macOS traffic-lights clearance. We reserve
        ~72px on the left so the window controls have room, and mark the whole
        strip as a drag region so the user can move the window from here.
      */}
      <div
        data-tauri-drag-region
        className="flex h-11 items-center gap-2 border-b border-border pl-[76px] pr-4"
      >
        <div className="flex size-6 items-center justify-center rounded-md bg-primary">
          <svg
            viewBox="0 0 24 24"
            className="size-3.5 text-primary-foreground"
            aria-hidden
          >
            <polygon points="4,18 12,4 20,18" fill="currentColor" />
          </svg>
        </div>
        <span className="text-sm font-semibold tracking-tight">MCP Hub</span>
      </div>

      <nav className="flex flex-col gap-0.5 p-2">
        {NAV.map((item) => {
          const Icon = item.icon;
          const active = current === item.route;
          return (
            <button
              key={item.route}
              type="button"
              onClick={() => onSelect(item.route)}
              className={cn(
                "group relative flex items-center gap-2.5 rounded-md px-2.5 py-1.5 text-sm transition-colors",
                active
                  ? "text-foreground"
                  : "text-muted-foreground hover:bg-accent/40 hover:text-foreground",
              )}
            >
              {active && (
                <motion.div
                  layoutId="sidebar-active"
                  className="absolute inset-0 -z-10 rounded-md bg-accent"
                  transition={{ type: "spring", stiffness: 600, damping: 40 }}
                />
              )}
              <Icon className="size-4" />
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>

      <div className="mt-auto p-3">
        <div className="rounded-lg border border-border bg-muted/40 p-3 text-xs text-muted-foreground">
          <div className="font-medium text-foreground">Local-first</div>
          <p className="mt-1 leading-relaxed">
            All data stays on this device. No cloud, no telemetry.
          </p>
        </div>
      </div>
    </aside>
  );
}
