import { useState, type ReactNode } from "react";

import { Sidebar, type Route } from "./sidebar";
import { Titlebar } from "./titlebar";

interface AppShellProps {
  render: (route: Route) => ReactNode;
}

/**
 * Two-pane application shell: sidebar (route switcher) + main content.
 * No router dependency — for a single-window desktop app, a tiny piece of
 * local state is enough and keeps the bundle lean.
 */
export function AppShell({ render }: AppShellProps) {
  const [route, setRoute] = useState<Route>("servers");

  return (
    <div className="flex h-full w-full overflow-hidden">
      <Sidebar current={route} onSelect={setRoute} />
      <div className="flex min-w-0 flex-1 flex-col">
        <Titlebar />
        <main className="min-h-0 flex-1 overflow-y-auto">{render(route)}</main>
      </div>
    </div>
  );
}
