import { describe, it, expect } from "vitest";
import { computeGroups, fmtMem, groupKey } from "./groupStats";
import type { WorkerStatus } from "./types";

function w(name: string, group: string | null, state: WorkerStatus["instances"][number]["state"]): WorkerStatus {
  return {
    name,
    group,
    tags: [],
    run_mode: { type: "daemon", concurrency: 1 },
    instances: [{ index: 0, state, restart_count: 0, pid: state === "running" ? 1 : null }],
  };
}

describe("groupStats", () => {
  it("groupKey falls back to Ungrouped", () => {
    expect(groupKey(w("a", "billing", "running"))).toBe("billing");
    expect(groupKey(w("a", null, "running"))).toBe("Ungrouped");
    expect(groupKey(w("a", "   ", "running"))).toBe("Ungrouped");
  });

  it("aggregates mem/cpu/processes and sorts billing before Ungrouped", () => {
    const workers = [
      w("queue", "billing", "running"),
      w("cron", "billing", "running"),
      w("loose", null, "running"),
    ];
    const latest = {
      queue: { 0: { cpu: 1.5, mem: 100 } },
      cron: { 0: { cpu: 2.5, mem: 200 } },
      loose: { 0: { cpu: 0.5, mem: 50 } },
    };
    const groups = computeGroups(workers, latest);
    expect(groups.map((g) => g.group)).toEqual(["billing", "Ungrouped"]);

    const billing = groups[0];
    expect(billing.mem).toBe(300);
    expect(billing.cpu).toBeCloseTo(4.0);
    expect(billing.processes).toBe(2);
    expect(billing.running).toBe(2);
    expect(billing.total).toBe(2);
  });

  it("only counts running instances toward processes", () => {
    const workers = [w("a", "g", "running"), w("b", "g", "stopped")];
    const latest = { a: { 0: { cpu: 1, mem: 10 } }, b: { 0: { cpu: 9, mem: 90 } } };
    const groups = computeGroups(workers, latest);
    // mem/cpu sum across instances that have a sample, regardless of state
    expect(groups[0].mem).toBe(100);
    expect(groups[0].cpu).toBe(10);
    // processes only counts running instances with a sample
    expect(groups[0].processes).toBe(1);
    expect(groups[0].running).toBe(1);
  });

  it("fmtMem formats MB and GB", () => {
    expect(fmtMem(300 * 1048576)).toBe("300 MB");
    expect(fmtMem(2 * 1073741824)).toBe("2.0 GB");
  });
});
