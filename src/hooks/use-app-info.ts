import { useEffect, useState } from "react";

import { api, isTauri } from "@/lib/tauri";
import type { AppInfo } from "@/types/mcp";

const FALLBACK: AppInfo = {
  version: "0.1.0",
  dataDir: "(browser dev)",
  sandboxEnforcement: "noop",
};

/**
 * Cached app metadata — version, data dir, sandbox enforcement label.
 * One fetch per app lifetime is plenty.
 */
export function useAppInfo(): AppInfo {
  const [info, setInfo] = useState<AppInfo>(FALLBACK);

  useEffect(() => {
    if (!isTauri) return;
    api.appInfo().then(setInfo).catch(() => {});
  }, []);

  return info;
}
