import type { InstanceState, WorkerStatus } from "../types";

/** A 3-bucket signal system: live / warming / down — plus neutral idle. */
export type Signal = "live" | "warm" | "down" | "idle";

export const SIGNAL_OF: Record<InstanceState, Signal> = {
  running: "live",
  starting: "warm",
  backoff: "warm",
  stopping: "warm",
  stopped: "idle",
  crashed: "down",
  failed: "down",
};

/** Severity order — first match wins when summarizing a worker's instances. */
const ORDER: InstanceState[] = [
  "failed",
  "crashed",
  "backoff",
  "starting",
  "stopping",
  "running",
  "stopped",
];

export function worstState(w: WorkerStatus): InstanceState {
  for (const s of ORDER) if (w.instances.some((i) => i.state === s)) return s;
  return "stopped";
}

/** Tailwind tokens per signal — dot color, accent rail, text, soft chip. */
export const SIGNAL_STYLE: Record<
  Signal,
  { dot: string; dotVar: string; rail: string; text: string; chip: string }
> = {
  live: {
    dot: "bg-emerald-500",
    dotVar: "rgba(16,185,129,0.55)",
    rail: "bg-emerald-500",
    text: "text-emerald-600 dark:text-emerald-400",
    chip: "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300 ring-emerald-500/25",
  },
  warm: {
    dot: "bg-amber-400",
    dotVar: "rgba(251,191,36,0.55)",
    rail: "bg-amber-400",
    text: "text-amber-600 dark:text-amber-400",
    chip: "bg-amber-400/10 text-amber-700 dark:text-amber-300 ring-amber-400/25",
  },
  down: {
    dot: "bg-rose-500",
    dotVar: "rgba(244,63,94,0.55)",
    rail: "bg-rose-500",
    text: "text-rose-600 dark:text-rose-400",
    chip: "bg-rose-500/10 text-rose-700 dark:text-rose-300 ring-rose-500/25",
  },
  idle: {
    dot: "bg-stone-400",
    dotVar: "rgba(168,162,158,0.5)",
    rail: "bg-stone-300 dark:bg-stone-600",
    text: "text-stone-500 dark:text-stone-400",
    chip: "bg-stone-500/10 text-stone-600 dark:text-stone-400 ring-stone-500/20",
  },
};

export function styleFor(state: InstanceState) {
  return SIGNAL_STYLE[SIGNAL_OF[state]];
}
