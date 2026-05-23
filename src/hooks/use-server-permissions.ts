import { useCallback, useEffect, useState } from "react";

import { api, isTauri } from "@/lib/tauri";
import type { PersistedPermission } from "@/types/permissions";

interface UseServerPermissionsResult {
  permissions: PersistedPermission[];
  loading: boolean;
  refresh: () => Promise<void>;
  setGranted: (id: number, granted: boolean) => Promise<void>;
}

export function useServerPermissions(
  serverId: string | null,
): UseServerPermissionsResult {
  const [permissions, setPermissions] = useState<PersistedPermission[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    if (!serverId) {
      setPermissions([]);
      return;
    }
    setLoading(true);
    if (isTauri) {
      try {
        const list = await api.listServerPermissions(serverId);
        setPermissions(list);
      } catch {
        setPermissions([]);
      }
    } else {
      setPermissions([]);
    }
    setLoading(false);
  }, [serverId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const setGranted = useCallback(
    async (id: number, granted: boolean) => {
      // Optimistic — flip locally, then call backend.
      setPermissions((prev) =>
        prev.map((p) => (p.id === id ? { ...p, granted } : p)),
      );
      if (!isTauri) return;
      try {
        if (granted) {
          await api.grantPermission(id);
        } else {
          await api.revokePermission(id);
        }
        await refresh();
      } catch {
        // Revert on failure.
        await refresh();
      }
    },
    [refresh],
  );

  return { permissions, loading, refresh, setGranted };
}
