import type { WorkerStatus } from "./types";

/**
 * Sum the latest per-instance CPU% and memory across every worker/instance.
 * Only instances that have reported a metric contribute.
 */
export function computeTotals(
  workers: WorkerStatus[],
  latest: Record<string, Record<number, { cpu: number; mem: number }>>,
): { cpu: number; mem: number; samples: number } {
  let cpu = 0;
  let mem = 0;
  let samples = 0;
  for (const w of workers) {
    const lm = latest[w.name] ?? {};
    for (const inst of w.instances) {
      const m = lm[inst.index];
      if (m) {
        cpu += m.cpu;
        mem += m.mem;
        samples += 1;
      }
    }
  }
  return { cpu, mem, samples };
}
