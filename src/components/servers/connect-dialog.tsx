import { useEffect, useState } from "react";
import { Check, Copy, ExternalLink, GitBranch } from "lucide-react";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { api, isTauri } from "@/lib/tauri";
import type { McpServer, ProxyConfig } from "@/types/mcp";

interface ConnectDialogProps {
  server: McpServer | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * Shows the JSON snippet a user pastes into Claude Desktop / Cursor /
 * Cline to route their MCP traffic through MCP Hub. With this connected,
 * Timeline shows real tool calls (not Demo Mode), Permissions are enforced
 * at the proxy boundary, and the sandbox layer kicks in on the spawned
 * child.
 */
export function ConnectDialog({ server, open, onOpenChange }: ConnectDialogProps) {
  const [config, setConfig] = useState<ProxyConfig | null>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open || !server) return;
    setError(null);
    setConfig(null);

    if (!isTauri) {
      // Provide a faux snippet in browser-only dev so designers can see it.
      setConfig({
        serverId: server.id,
        serverName: server.name,
        proxyPath: "/path/to/mcp-hub-proxy",
        sockPath: "~/Library/Application Support/MCP Hub/proxy.sock",
        snippet: dummySnippet(server),
      });
      return;
    }
    api
      .getProxyConfig(server.id)
      .then(setConfig)
      .catch((e) => setError(e instanceof Error ? e.message : String(e)));
  }, [open, server]);

  const handleCopy = async () => {
    if (!config) return;
    try {
      await navigator.clipboard.writeText(config.snippet);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    } catch {
      setError("Could not copy to clipboard");
    }
  };

  if (!server) return null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <div className="flex items-center gap-3">
            <div className="flex size-9 items-center justify-center rounded-lg border border-primary/30 bg-primary/10 text-primary">
              <GitBranch className="size-4" />
            </div>
            <div>
              <DialogTitle>Connect {server.name} to your AI client</DialogTitle>
              <DialogDescription>
                Route traffic through MCP Hub to enable Timeline observability,
                runtime permission checks, and sandbox enforcement.
              </DialogDescription>
            </div>
          </div>
        </DialogHeader>

        <div className="space-y-4">
          {error && (
            <div className="rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive">
              {error}
            </div>
          )}

          <div>
            <div className="mb-1.5 flex items-center justify-between">
              <span className="text-xs font-medium text-muted-foreground">
                Add to <code className="rounded bg-muted/50 px-1 py-0.5">claude_desktop_config.json</code>
              </span>
              <Button
                size="sm"
                variant={copied ? "secondary" : "default"}
                onClick={() => void handleCopy()}
                className="h-7 text-xs"
                disabled={!config}
              >
                {copied ? (
                  <>
                    <Check className="size-3" /> Copied
                  </>
                ) : (
                  <>
                    <Copy className="size-3" /> Copy
                  </>
                )}
              </Button>
            </div>
            <pre className="max-h-64 overflow-auto rounded-md border border-border bg-[hsl(240_10%_3%)] p-3 font-mono text-[11px] leading-relaxed text-foreground/85">
              {config?.snippet ?? "Loading…"}
            </pre>
          </div>

          <div className="rounded-lg border border-border bg-muted/20 p-3 text-[11px] text-muted-foreground">
            <p>
              The config file lives at{" "}
              <code className="font-mono">
                ~/Library/Application Support/Claude/claude_desktop_config.json
              </code>{" "}
              on macOS. Restart Claude Desktop after editing. From then on,
              every <code>tools/call</code> shows up live in the Timeline page
              with real timings.
            </p>
          </div>

          <a
            href="https://modelcontextprotocol.io/quickstart/user"
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-1 text-[11px] text-primary hover:underline"
          >
            <ExternalLink className="size-3" />
            MCP client configuration docs
          </a>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function dummySnippet(server: McpServer): string {
  const key = server.name.toLowerCase().replace(/[^a-z0-9_-]/g, "-");
  return `{
  "mcpServers": {
    "${key}": {
      "command": "/path/to/mcp-hub-proxy",
      "args": ["${server.id}"]
    }
  }
}`;
}
