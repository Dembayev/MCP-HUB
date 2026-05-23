import { AnimatePresence, motion } from "framer-motion";
import { Check, ShieldAlert, ShieldCheck, ShieldX } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useApprovals } from "@/hooks/use-approvals";
import { cn } from "@/lib/utils";
import type { RiskLevel } from "@/types/approval";

/**
 * Runtime approval prompt. Floats above all pages; mounted once at the
 * AppShell level. Non-dismissable — the user MUST pick one of the three
 * buttons. Closing the modal otherwise would leave the proxy hanging on the
 * oneshot; the backend treats a dropped channel as a fail-safe Deny, but
 * the UX should never tempt the user toward that path.
 *
 * Visual contract (per launch_guardrails): the prompt IS the "Approve" verb
 * of the launch story. Make it feel like a real OS permission dialog —
 * authoritative, calm, dense with the facts. macOS / Android TCC vibe.
 */
export function ApprovalModal() {
  const { current, pending, resolve } = useApprovals();

  return (
    <AnimatePresence>
      {current && (
        <motion.div
          key="approval-overlay"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.15 }}
          className="fixed inset-0 z-50 flex items-center justify-center bg-background/70 backdrop-blur-sm"
        >
          <motion.div
            key={current.id}
            initial={{ opacity: 0, scale: 0.95, y: 12 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.97, y: 4 }}
            transition={{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }}
            className="w-full max-w-md rounded-2xl border border-border bg-card shadow-2xl"
          >
            <Header risk={current.risk} serverName={current.serverName} />
            <Body
              tool={current.tool}
              target={current.target}
              scope={current.scope}
            />
            <Buttons
              risk={current.risk}
              onAllowOnce={() => void resolve("allow_once")}
              onAllowSession={() => void resolve("allow_session")}
              onDeny={() => void resolve("deny")}
            />
            {pending > 1 && <QueueIndicator total={pending} />}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

// ---------------------------------------------------------------------------

function Header({
  risk,
  serverName,
}: {
  risk: RiskLevel;
  serverName: string;
}) {
  const Icon = risk === "high" ? ShieldAlert : risk === "medium" ? ShieldCheck : ShieldCheck;
  return (
    <div className="flex items-start gap-3 border-b border-border/60 px-5 py-4">
      <div
        className={cn(
          "flex size-9 items-center justify-center rounded-xl border",
          risk === "high" &&
            "border-destructive/40 bg-destructive/10 text-destructive",
          risk === "medium" &&
            "border-amber-400/40 bg-amber-400/10 text-amber-300",
          risk === "low" &&
            "border-emerald-500/30 bg-emerald-500/10 text-emerald-300",
        )}
      >
        <Icon className="size-4" />
      </div>
      <div className="min-w-0 flex-1">
        <h2 className="text-sm font-semibold">Approve action?</h2>
        <p className="mt-0.5 truncate text-[12px] text-muted-foreground">
          <span className="font-mono">{serverName}</span> wants permission
        </p>
      </div>
      <RiskBadge risk={risk} />
    </div>
  );
}

function Body({
  tool,
  target,
  scope,
}: {
  tool: string;
  target: string | null;
  scope: string;
}) {
  return (
    <div className="px-5 py-4">
      <div className="space-y-2">
        <Field label="Tool">
          <span className="font-mono text-foreground">{tool}</span>
        </Field>
        {target && (
          <Field label="Target">
            <span className="break-all font-mono text-foreground/90">
              {target}
            </span>
          </Field>
        )}
        <Field label="Scope">
          <span className="font-mono text-foreground/90">{scope}</span>
        </Field>
      </div>
      <p className="mt-4 text-[12px] leading-relaxed text-muted-foreground">
        MCP Hub paused this request so you can decide. Your choice is recorded
        in the session trace and reflected in Replay.
      </p>
    </div>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-baseline gap-2 text-[12px]">
      <span className="w-14 shrink-0 text-[10px] font-medium uppercase tracking-wider text-muted-foreground/70">
        {label}
      </span>
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  );
}

function Buttons({
  risk,
  onAllowOnce,
  onAllowSession,
  onDeny,
}: {
  risk: RiskLevel;
  onAllowOnce: () => void;
  onAllowSession: () => void;
  onDeny: () => void;
}) {
  return (
    <div className="flex flex-col gap-1.5 border-t border-border/60 px-5 py-3">
      <Button
        variant="default"
        size="sm"
        className="w-full justify-start"
        onClick={onAllowOnce}
      >
        <Check className="size-3.5" />
        Allow once
      </Button>
      <Button
        variant="secondary"
        size="sm"
        className="w-full justify-start"
        onClick={onAllowSession}
      >
        <Check className="size-3.5" />
        Always allow {risk === "high" ? "(persisted)" : ""}
      </Button>
      <Button
        variant="ghost"
        size="sm"
        className="w-full justify-start text-destructive hover:bg-destructive/10 hover:text-destructive"
        onClick={onDeny}
      >
        <ShieldX className="size-3.5" />
        Deny
      </Button>
    </div>
  );
}

function RiskBadge({ risk }: { risk: RiskLevel }) {
  return (
    <span
      className={cn(
        "shrink-0 rounded-md border px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider",
        risk === "high" &&
          "border-destructive/40 bg-destructive/15 text-destructive",
        risk === "medium" &&
          "border-amber-400/40 bg-amber-400/15 text-amber-300",
        risk === "low" &&
          "border-emerald-500/30 bg-emerald-500/10 text-emerald-300",
      )}
    >
      {risk}
    </span>
  );
}

function QueueIndicator({ total }: { total: number }) {
  return (
    <div className="border-t border-border/60 px-5 py-2 text-center text-[11px] text-muted-foreground">
      {total - 1} more pending after this one
    </div>
  );
}
