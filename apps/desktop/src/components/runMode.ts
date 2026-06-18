import type { RunModeAny } from "../types";

/** Short label for a worker's run mode, e.g. "daemon ×4", "cron 0 3 * * *", "batch ×5". */
export function runModeLabel(rm: RunModeAny): string {
  switch (rm?.type) {
    case "scheduled":
      return "cron" in rm.schedule
        ? `cron ${rm.schedule.cron}`
        : `every ${rm.schedule.interval_secs}s`;
    case "batch":
      return `batch ×${rm.run_count}`;
    default:
      return `daemon ×${rm.concurrency ?? 1}`;
  }
}

/** The right-hand "conc" metric value, mode-aware. */
export function runModeMetric(rm: RunModeAny): { label: string; value: string } {
  switch (rm?.type) {
    case "scheduled":
      return {
        label: "schedule",
        value: "cron" in rm.schedule ? rm.schedule.cron : `${rm.schedule.interval_secs}s`,
      };
    case "batch":
      return { label: "runs", value: `${rm.run_count}×` };
    default:
      return { label: "conc", value: `×${rm.concurrency ?? 1}` };
  }
}
