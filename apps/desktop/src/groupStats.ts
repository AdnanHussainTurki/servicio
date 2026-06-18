import type { WorkerStatus } from "./types";

export interface GroupStat { group: string; mem: number; cpu: number; processes: number; running: number; total: number; restarts: number; workers: WorkerStatus[]; }

export function groupKey(w: WorkerStatus): string { return w.group?.trim() || "Ungrouped"; }

/** Aggregate workers into per-group stats using the latest per-instance metrics. */
export function computeGroups(
  workers: WorkerStatus[],
  latest: Record<string, Record<number, { cpu: number; mem: number }>>,
): GroupStat[] {
  const map = new Map<string, GroupStat>();
  for (const w of workers) {
    const k = groupKey(w);
    let g = map.get(k);
    if (!g) { g = { group: k, mem: 0, cpu: 0, processes: 0, running: 0, total: 0, restarts: 0, workers: [] }; map.set(k, g); }
    g.workers.push(w);
    g.total += 1;
    const lm = latest[w.name] ?? {};
    for (const inst of w.instances) {
      if (inst.state === "running") g.running += 1;
      g.restarts += inst.restart_count;
      const m = lm[inst.index];
      if (m) { g.mem += m.mem; g.cpu += m.cpu; if (inst.state === "running") g.processes += 1; }
    }
  }
  // Sort by memory desc (heaviest first); "Ungrouped" always last.
  return [...map.values()].sort((a, b) => {
    if (a.group === "Ungrouped") return 1;
    if (b.group === "Ungrouped") return -1;
    return b.mem - a.mem;
  });
}

export function fmtMem(bytes: number): string {
  const mb = bytes / 1048576;
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${mb.toFixed(0)} MB`;
}
