import { useState } from "react";
import { motion } from "framer-motion";
import { GitBranch, Play, Square, Terminal, Trash2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { StatusDot } from "./status-dot";
import { ConnectDialog } from "./connect-dialog";
import { cn } from "@/lib/utils";
import { formatRelativeTime } from "@/lib/utils";
import type { McpServer } from "@/types/mcp";

interface ServerCardProps {
  server: McpServer;
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onRemove: (id: string) => void;
}

export function ServerCard({ server, onStart, onStop, onRemove }: ServerCardProps) {
  const running = server.status === "running" || server.status === "starting";
  const [connectOpen, setConnectOpen] = useState(false);

  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.18, ease: "easeOut" }}
    >
      <Card className="group flex h-full flex-col gap-4 p-5 transition-colors hover:border-border/80 hover:bg-card/80">
        <div className="flex items-start gap-3">
          <div className="flex size-10 shrink-0 items-center justify-center rounded-lg border border-border bg-muted/40">
            <Terminal className="size-4 text-muted-foreground" />
          </div>

          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <h3 className="truncate font-medium tracking-tight">{server.name}</h3>
              <StatusDot status={server.status} />
            </div>
            <p className="mt-0.5 line-clamp-2 text-xs text-muted-foreground">
              {server.description || "No description"}
            </p>
          </div>

          <Button
            variant="ghost"
            size="icon"
            className="size-7 opacity-0 transition-opacity group-hover:opacity-100"
            onClick={() => onRemove(server.id)}
            aria-label="Remove server"
          >
            <Trash2 className="size-3.5" />
          </Button>
        </div>

        <div className="flex flex-wrap items-center gap-1.5">
          <Badge variant="outline" className="font-mono">
            {server.transport}
          </Badge>
          {server.version && (
            <Badge variant="secondary">v{server.version}</Badge>
          )}
          {server.source && (
            <Badge variant="outline" className="capitalize">
              {server.source}
            </Badge>
          )}
        </div>

        <div className="flex items-center justify-between border-t border-border/60 pt-3">
          <span className="text-xs text-muted-foreground">
            Updated {formatRelativeTime(server.updatedAt)}
          </span>
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="sm"
              className="h-7 px-2 text-xs"
              onClick={() => setConnectOpen(true)}
              title="Connect this server to your AI client via MCP Hub proxy"
            >
              <GitBranch className="size-3" />
              Connect
            </Button>
            {running ? (
              <Button
                size="sm"
                variant="secondary"
                onClick={() => onStop(server.id)}
              >
                <Square className="size-3.5" />
                Stop
              </Button>
            ) : (
              <Button
                size="sm"
                onClick={() => onStart(server.id)}
                className={cn(
                  server.status === "crashed" && "bg-destructive hover:bg-destructive/90",
                )}
              >
                <Play className="size-3.5" />
                {server.status === "crashed" ? "Restart" : "Start"}
              </Button>
            )}
          </div>
        </div>
      </Card>

      <ConnectDialog
        server={server}
        open={connectOpen}
        onOpenChange={setConnectOpen}
      />
    </motion.div>
  );
}
