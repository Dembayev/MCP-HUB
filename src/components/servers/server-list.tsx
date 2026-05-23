import { AnimatePresence } from "framer-motion";

import { ServerCard } from "./server-card";
import type { McpServer } from "@/types/mcp";

interface ServerListProps {
  servers: McpServer[];
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onRemove: (id: string) => void;
}

export function ServerList({ servers, onStart, onStop, onRemove }: ServerListProps) {
  return (
    <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
      <AnimatePresence mode="popLayout">
        {servers.map((server) => (
          <ServerCard
            key={server.id}
            server={server}
            onStart={onStart}
            onStop={onStop}
            onRemove={onRemove}
          />
        ))}
      </AnimatePresence>
    </div>
  );
}
