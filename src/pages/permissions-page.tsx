import { useEffect, useState } from "react";
import { Check, ShieldCheck, X } from "lucide-react";

import { SandboxBadge } from "@/components/permissions/sandbox-badge";
import { ServerPermsPicker } from "@/components/permissions/server-perms-picker";
import { PermissionList } from "@/components/marketplace/permission-list";
import { Button } from "@/components/ui/button";
import { useAppInfo } from "@/hooks/use-app-info";
import { useMcpServers } from "@/hooks/use-mcp-servers";
import { useServerPermissions } from "@/hooks/use-server-permissions";
import { cn } from "@/lib/utils";
import type {
  PermissionScope,
  RequiredPermission,
} from "@/types/marketplace";
import type { PersistedPermission } from "@/types/permissions";

export function PermissionsPage() {
  const { servers } = useMcpServers();
  const appInfo = useAppInfo();

  const [selectedId, setSelectedId] = useState<string | null>(null);
  useEffect(() => {
    if (selectedId && servers.some((s) => s.id === selectedId)) return;
    setSelectedId(servers[0]?.id ?? null);
  }, [servers, selectedId]);

  const selected = servers.find((s) => s.id === selectedId) ?? null;
  const { permissions, loading, setGranted } = useServerPermissions(selectedId);

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-border px-6 py-4">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">Permissions</h1>
          <p className="text-xs text-muted-foreground">
            What each server is allowed to do — and whether the OS is enforcing it.
          </p>
        </div>
        <SandboxBadge enforcement={appInfo.sandboxEnforcement} />
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[240px_1fr]">
        <aside className="min-h-0 overflow-y-auto border-r border-border bg-card/40">
          <ServerPermsPicker
            servers={servers}
            selectedId={selectedId}
            onSelect={setSelectedId}
          />
        </aside>

        <section className="min-h-0 overflow-y-auto px-6 py-6 animate-fade-in">
          {!selected ? (
            <EmptyDetail />
          ) : permissions.length === 0 && !loading ? (
            <ServerSummary selected={selected}>
              <NoPermissions />
            </ServerSummary>
          ) : (
            <ServerSummary selected={selected}>
              <PermissionTable
                permissions={permissions}
                onToggle={(p) => void setGranted(p.id, !p.granted)}
              />

              <div className="mt-6 rounded-lg border border-border bg-muted/20 p-4 text-xs text-muted-foreground">
                <div className="mb-1 flex items-center gap-2 font-medium text-foreground">
                  <ShieldCheck className="size-3.5" /> How enforcement works
                </div>
                <p>
                  Granted scopes are compiled into a per-server sandbox profile
                  on next start. On macOS we use{" "}
                  <span className="font-mono">sandbox-exec</span> (SBPL) to
                  apply filesystem and network rules. Other platforms record
                  the grants but don't apply them at the OS level yet — that's
                  on the roadmap.
                </p>
              </div>
            </ServerSummary>
          )}
        </section>
      </div>
    </div>
  );
}

function ServerSummary({
  selected,
  children,
}: {
  selected: { name: string; description: string };
  children: React.ReactNode;
}) {
  return (
    <>
      <div className="mb-5">
        <h2 className="text-lg font-semibold tracking-tight">{selected.name}</h2>
        <p className="text-sm text-muted-foreground">
          {selected.description || "No description"}
        </p>
      </div>
      {children}
    </>
  );
}

function EmptyDetail() {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
      <ShieldCheck className="size-6 text-muted-foreground" />
      <p className="text-sm text-muted-foreground">
        Pick a server from the left to review and revoke its permissions.
      </p>
    </div>
  );
}

function NoPermissions() {
  return (
    <PermissionList permissions={[]} />
  );
}

interface PermissionTableProps {
  permissions: PersistedPermission[];
  onToggle: (p: PersistedPermission) => void;
}

function PermissionTable({ permissions, onToggle }: PermissionTableProps) {
  // Reuse the rich marketplace PermissionList look but layer toggles on top
  // by rendering rows ourselves — same icon vocabulary so the install dialog
  // and this page feel like the same surface.
  const required: RequiredPermission[] = permissions.map((p) => ({
    scope: p.scope as PermissionScope,
    target: p.target ?? undefined,
    reason: p.reason ?? "Granted at install.",
  }));

  // For the toggle column we just iterate `permissions` in parallel.
  return (
    <div className="grid grid-cols-1 gap-3">
      <PermissionList permissions={required} />
      <ul className="divide-y divide-border rounded-lg border border-border">
        {permissions.map((p) => (
          <li
            key={p.id}
            className={cn(
              "flex items-center gap-3 px-3 py-2 text-sm",
              !p.granted && "opacity-60",
            )}
          >
            <span className="flex-1 truncate font-mono text-xs">
              {p.scope}
              {p.target ? (
                <span className="text-muted-foreground"> · {p.target}</span>
              ) : null}
            </span>
            <Button
              size="sm"
              variant={p.granted ? "secondary" : "default"}
              onClick={() => onToggle(p)}
              className="h-7 text-xs"
            >
              {p.granted ? (
                <>
                  <X className="size-3" /> Revoke
                </>
              ) : (
                <>
                  <Check className="size-3" /> Grant
                </>
              )}
            </Button>
          </li>
        ))}
      </ul>
    </div>
  );
}
