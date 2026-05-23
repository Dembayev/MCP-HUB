import type { LucideIcon } from "lucide-react";

interface PlaceholderPageProps {
  title: string;
  description: string;
  icon: LucideIcon;
}

/**
 * Generic "coming soon" page used for routes we haven't built UI for yet
 * (marketplace, activity, permissions, settings). Replacing each of these
 * with real screens is what turns the scaffold into the MVP.
 */
export function PlaceholderPage({
  title,
  description,
  icon: Icon,
}: PlaceholderPageProps) {
  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-border px-6 py-4">
        <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
        <p className="text-xs text-muted-foreground">{description}</p>
      </div>

      <div className="flex flex-1 items-center justify-center p-10 animate-fade-in">
        <div className="flex max-w-sm flex-col items-center gap-4 text-center">
          <div className="flex size-14 items-center justify-center rounded-2xl border border-border bg-muted/30">
            <Icon className="size-6 text-muted-foreground" />
          </div>
          <h2 className="text-lg font-semibold tracking-tight">Coming soon</h2>
          <p className="text-sm text-muted-foreground">
            This view is part of the MVP roadmap. The scaffold underneath is
            ready — wire up data and ship.
          </p>
        </div>
      </div>
    </div>
  );
}
