import { Settings } from "lucide-react";

import { AppShell } from "@/components/layout/app-shell";
import { ActivityPage } from "@/pages/activity-page";
import { MarketplacePage } from "@/pages/marketplace-page";
import { PermissionsPage } from "@/pages/permissions-page";
import { PlaceholderPage } from "@/pages/placeholder-page";
import { ServersPage } from "@/pages/servers-page";
import { TimelinePage } from "@/pages/timeline-page";

export default function App() {
  return (
    <AppShell
      render={(route) => {
        switch (route) {
          case "servers":
            return <ServersPage />;
          case "marketplace":
            return <MarketplacePage />;
          case "timeline":
            return <TimelinePage />;
          case "activity":
            return <ActivityPage />;
          case "permissions":
            return <PermissionsPage />;
          case "settings":
            return (
              <PlaceholderPage
                title="Settings"
                description="App-wide preferences, integrations, and updates."
                icon={Settings}
              />
            );
        }
      }}
    />
  );
}
