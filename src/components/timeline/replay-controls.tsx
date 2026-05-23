import { Pause, Play, RotateCcw, ShieldX, SkipBack, SkipForward } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { REPLAY_SPEEDS, type ReplaySpeed } from "@/hooks/use-replay";

/**
 * Playback control bar — play/pause, step ±, speed selector, jump-to-denial.
 *
 * Layout note: this sits BELOW the scrubber, ABOVE the action list.
 * Keep it compact — every pixel competing with the action stream is a loss.
 */
export function ReplayControls({
  playing,
  speed,
  position,
  total,
  positionMs,
  totalMs,
  onToggle,
  onStepBack,
  onStepForward,
  onSetSpeed,
  onJumpToNextDenial,
  onReset,
  hasDenials,
}: {
  playing: boolean;
  speed: ReplaySpeed;
  position: number;
  total: number;
  positionMs: number;
  totalMs: number;
  onToggle: () => void;
  onStepBack: () => void;
  onStepForward: () => void;
  onSetSpeed: (speed: ReplaySpeed) => void;
  onJumpToNextDenial: () => void;
  onReset: () => void;
  hasDenials: boolean;
}) {
  const atStart = position <= 0;
  const atEnd = position >= total - 1;

  return (
    <div className="flex flex-wrap items-center justify-between gap-2 px-1 py-1.5">
      <div className="flex items-center gap-1">
        <Button
          size="sm"
          variant="ghost"
          className="h-7 px-2"
          onClick={onReset}
          aria-label="reset to start"
          disabled={total === 0}
        >
          <RotateCcw className="size-3.5" />
        </Button>
        <Button
          size="sm"
          variant="ghost"
          className="h-7 px-2"
          onClick={onStepBack}
          aria-label="step back"
          disabled={atStart}
        >
          <SkipBack className="size-3.5" />
        </Button>
        <Button
          size="sm"
          variant={playing ? "secondary" : "default"}
          className="h-7 px-3"
          onClick={onToggle}
          aria-label={playing ? "pause" : "play"}
          disabled={total === 0}
        >
          {playing ? (
            <>
              <Pause className="size-3.5" /> Pause
            </>
          ) : (
            <>
              <Play className="size-3.5" /> Play
            </>
          )}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          className="h-7 px-2"
          onClick={onStepForward}
          aria-label="step forward"
          disabled={atEnd}
        >
          <SkipForward className="size-3.5" />
        </Button>

        {hasDenials && (
          <Button
            size="sm"
            variant="ghost"
            className="ml-2 h-7 px-2 text-destructive/90 hover:text-destructive"
            onClick={onJumpToNextDenial}
            aria-label="jump to next denial"
          >
            <ShieldX className="size-3.5" /> Next denial
          </Button>
        )}
      </div>

      <div className="flex items-center gap-3 text-[11px] text-muted-foreground">
        <span className="font-mono tabular-nums">
          {formatMs(positionMs)} / {formatMs(totalMs)}
        </span>
        <span className="font-mono tabular-nums">
          #{position}/{Math.max(0, total - 1)}
        </span>
        <SpeedSelector speed={speed} onChange={onSetSpeed} />
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------

function SpeedSelector({
  speed,
  onChange,
}: {
  speed: ReplaySpeed;
  onChange: (s: ReplaySpeed) => void;
}) {
  const options: ReplaySpeed[] = [...REPLAY_SPEEDS, "instant"];
  return (
    <div className="flex items-center overflow-hidden rounded-md border border-border bg-card/40">
      {options.map((opt) => (
        <button
          key={String(opt)}
          type="button"
          onClick={() => onChange(opt)}
          className={cn(
            "px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wider transition-colors",
            opt === speed
              ? "bg-foreground/10 text-foreground"
              : "text-muted-foreground hover:bg-foreground/5 hover:text-foreground/80",
          )}
        >
          {opt === "instant" ? "∞" : `${opt}x`}
        </button>
      ))}
    </div>
  );
}

function formatMs(ms: number): string {
  if (ms < 1_000) return `${ms} ms`;
  if (ms < 60_000) return `${(ms / 1_000).toFixed(1)} s`;
  const m = Math.floor(ms / 60_000);
  const s = Math.floor((ms - m * 60_000) / 1_000);
  return `${m}m${s}s`;
}
