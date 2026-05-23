/**
 * Replay engine hook — the moat layer that turns Timeline from inspector
 * into trace debugger.
 *
 * Design contract (per `mcp_hub_launch_guardrails` memory):
 *
 * - **Deterministic playback.** Two replays of the same trace are byte-for-byte
 *   identical in timing because we use `ts_mono_ns` deltas, NOT wall clock.
 * - **`seq` is logical order.** Reader already sorted; we index into the
 *   pre-sorted array by position (0..N-1) for O(1) advance and seek.
 * - **Replay is the moat, not the cinema.** This hook ships scrubber +
 *   step-through + speed control. Video rendering is explicitly deferred.
 *
 * Lifecycle:
 *
 * 1. Caller passes the current `actions` array. Whenever the array identity
 *    changes (new trace selected), playback resets to position 0, paused.
 * 2. `play()` schedules the next action via `setTimeout` based on the inter-
 *    action `ts_mono_ns` delta divided by `speed`.
 * 3. `seek(i)` clamps to range, cancels any pending tick, updates position.
 * 4. On unmount: cancel pending tick.
 */

import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MutableRefObject,
} from "react";

import type { Action } from "@/types/session";

/** Discrete speed steps shown in the UI. `instant` advances every microtask. */
export const REPLAY_SPEEDS = [0.5, 1, 2, 5, 10] as const;
export type ReplaySpeed = (typeof REPLAY_SPEEDS)[number] | "instant";

/** Upper bound on how long we'll wait between actions, regardless of trace
 *  timing. Some traces have multi-second gaps; a real-time replay of those is
 *  boring and breaks demo flow. Cap helps keep playback feeling alive. */
const MAX_INTER_ACTION_MS = 2_000;

/** Lower bound — when speed=10x or trace is dense, we don't want to fire
 *  hundreds of setTimeouts per second. Bottoms out at one frame. */
const MIN_INTER_ACTION_MS = 16;

export interface ReplayState {
  /** Index into the actions array (0..N-1). `-1` when there are no actions. */
  position: number;
  /** Total action count. Cached so consumers can render N without re-reading. */
  total: number;
  /** True while the playback timer is running. */
  playing: boolean;
  /** Current playback speed multiplier (or "instant"). */
  speed: ReplaySpeed;
  /** The action at `position`, or `null` when empty. */
  current: Action | null;
  /** Cumulative monotonic offset from trace start at the current position. */
  positionMs: number;
  /** Total trace duration in ms (last action's mono_ns + duration_ns). */
  totalMs: number;
}

export interface ReplayControls {
  play: () => void;
  pause: () => void;
  toggle: () => void;
  /** Jump to a specific action index. Clamps to range. Pauses playback. */
  seek: (index: number) => void;
  /** Advance one action regardless of timing. */
  stepForward: () => void;
  /** Step backward one action. */
  stepBackward: () => void;
  /** Set playback speed. */
  setSpeed: (speed: ReplaySpeed) => void;
  /** Jump to the next action whose outcome is "denied". No-op if none. */
  jumpToNextDenial: () => void;
  /** Reset position to 0 (start of trace) and pause. */
  reset: () => void;
}

export function useReplay(actions: Action[]): ReplayState & ReplayControls {
  const [position, setPosition] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState<ReplaySpeed>(1);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Total trace duration — last action's monotonic position + its duration.
  // Cached as a number so the scrubber doesn't recompute on every tick.
  const totalMs = useMemo(() => {
    const last = actions[actions.length - 1];
    if (!last) return 0;
    const endNs = last.ts_mono_ns + (last.duration_ns ?? 0);
    return Math.round(endNs / 1_000_000);
  }, [actions]);

  // Reset playback when the actions array identity changes (new trace).
  useEffect(() => {
    cancelTimer(timerRef);
    setPosition(0);
    setPlaying(false);
  }, [actions]);

  // Cleanup on unmount.
  useEffect(() => {
    return () => cancelTimer(timerRef);
  }, []);

  // Core advance step. Returns true if there was a next action to advance to.
  const advance = useCallback(() => {
    setPosition((prev) => {
      const next = prev + 1;
      if (next >= actions.length) {
        // Reached the end — stop playing.
        setPlaying(false);
        return prev;
      }
      return next;
    });
  }, [actions.length]);

  // Schedule the next tick whenever we're playing and have headroom.
  useEffect(() => {
    cancelTimer(timerRef);
    if (!playing) return;
    if (position >= actions.length - 1) {
      setPlaying(false);
      return;
    }

    const delayMs = interActionDelay(actions, position, speed);
    timerRef.current = setTimeout(() => {
      advance();
    }, delayMs);

    return () => cancelTimer(timerRef);
  }, [playing, position, speed, actions, advance]);

  // -----------------------------------------------------------------------
  // Controls (stable references so consumers can pass to onClick directly).
  // -----------------------------------------------------------------------

  const play = useCallback(() => {
    if (actions.length === 0) return;
    // If we're at the end, restart from the beginning on play.
    setPosition((prev) => (prev >= actions.length - 1 ? 0 : prev));
    setPlaying(true);
  }, [actions.length]);

  const pause = useCallback(() => {
    setPlaying(false);
  }, []);

  const toggle = useCallback(() => {
    if (playing) pause();
    else play();
  }, [playing, play, pause]);

  const seek = useCallback(
    (index: number) => {
      const clamped = clamp(index, 0, Math.max(0, actions.length - 1));
      setPosition(clamped);
      setPlaying(false);
    },
    [actions.length],
  );

  const stepForward = useCallback(() => {
    setPlaying(false);
    setPosition((prev) => clamp(prev + 1, 0, Math.max(0, actions.length - 1)));
  }, [actions.length]);

  const stepBackward = useCallback(() => {
    setPlaying(false);
    setPosition((prev) => clamp(prev - 1, 0, Math.max(0, actions.length - 1)));
  }, [actions.length]);

  const reset = useCallback(() => {
    setPlaying(false);
    setPosition(0);
  }, []);

  const jumpToNextDenial = useCallback(() => {
    const startFrom = position + 1;
    const next = actions.findIndex(
      (a, i) => i >= startFrom && a.outcome === "denied",
    );
    if (next >= 0) {
      setPosition(next);
      setPlaying(false);
    }
  }, [actions, position]);

  // -----------------------------------------------------------------------
  // Derived state
  // -----------------------------------------------------------------------

  const current = actions[position] ?? null;
  const positionMs =
    current !== null ? Math.round(current.ts_mono_ns / 1_000_000) : 0;

  return {
    position,
    total: actions.length,
    playing,
    speed,
    current,
    positionMs,
    totalMs,
    play,
    pause,
    toggle,
    seek,
    stepForward,
    stepBackward,
    setSpeed,
    jumpToNextDenial,
    reset,
  };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function cancelTimer(
  ref: MutableRefObject<ReturnType<typeof setTimeout> | null>,
) {
  if (ref.current !== null) {
    clearTimeout(ref.current);
    ref.current = null;
  }
}

function clamp(value: number, lo: number, hi: number): number {
  if (value < lo) return lo;
  if (value > hi) return hi;
  return value;
}

/**
 * Compute the delay (ms) before advancing from `actions[position]` to
 * `actions[position + 1]`, given the playback speed multiplier.
 *
 * Uses the monotonic timing baked into the trace (`ts_mono_ns`) so playback
 * is deterministic — the same trace replays identically on any machine.
 *
 * Clamps to [MIN_INTER_ACTION_MS, MAX_INTER_ACTION_MS] so playback always
 * feels alive (no multi-second dead air) and doesn't fire hundreds of
 * timers per second on very dense traces at high speeds.
 */
function interActionDelay(
  actions: Action[],
  position: number,
  speed: ReplaySpeed,
): number {
  if (speed === "instant") return 0;

  const current = actions[position];
  const next = actions[position + 1];
  if (!current || !next) return MIN_INTER_ACTION_MS;

  const deltaNs = Math.max(0, next.ts_mono_ns - current.ts_mono_ns);
  const realMs = deltaNs / 1_000_000;
  const scaledMs = realMs / speed;
  return Math.max(
    MIN_INTER_ACTION_MS,
    Math.min(MAX_INTER_ACTION_MS, Math.round(scaledMs)),
  );
}
