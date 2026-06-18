import { describe, it, expect, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { useStore } from "../store";
import { DashboardGraphs } from "./DashboardGraphs";
import { computeTotals } from "../dashboardTotals";
import type { WorkerStatus } from "../types";

const MB = 1048576;

describe("computeTotals", () => {
  it("sums cpu and mem across all workers and instances with metrics", () => {
    const workers: WorkerStatus[] = [
      {
        name: "a",
        run_mode: { type: "daemon", concurrency: 2 },
        instances: [
          { index: 0, state: "running", restart_count: 0, pid: 1 },
          { index: 1, state: "running", restart_count: 0, pid: 2 },
        ],
      },
    ];
    const latest = {
      a: { 0: { cpu: 5, mem: 100 * MB }, 1: { cpu: 7.5, mem: 200 * MB } },
    };
    const t = computeTotals(workers, latest);
    expect(t.cpu).toBeCloseTo(12.5);
    expect(t.mem).toBe(300 * MB);
    expect(t.samples).toBe(2);
  });
});

describe("DashboardGraphs", () => {
  beforeEach(() => useStore.getState().reset());

  it("shows total cpu and memory across the fleet", () => {
    useStore.getState().setWorkers([
      {
        name: "queue",
        run_mode: { type: "daemon", concurrency: 2 },
        instances: [
          { index: 0, state: "running", restart_count: 0, pid: 10 },
          { index: 1, state: "running", restart_count: 0, pid: 11 },
        ],
      },
    ]);
    // two instances reporting 150 MB / 10% each -> 300 MB, 20%
    useStore.getState().applyEvent({ kind: "metric", worker: "queue", instance: 0, ts: 1, cpu: 10, mem: 150 * MB });
    useStore.getState().applyEvent({ kind: "metric", worker: "queue", instance: 1, ts: 1, cpu: 10, mem: 150 * MB });

    render(<DashboardGraphs />);
    expect(screen.getByText("300 MB")).toBeInTheDocument();
    expect(screen.getByText("20.0%")).toBeInTheDocument();
  });

  it("shows an awaiting-telemetry placeholder before any metric", () => {
    useStore.getState().setWorkers([
      {
        name: "idle",
        run_mode: { type: "daemon", concurrency: 1 },
        instances: [{ index: 0, state: "running", restart_count: 0, pid: 5 }],
      },
    ]);
    render(<DashboardGraphs />);
    expect(screen.getByText(/awaiting telemetry/i)).toBeInTheDocument();
  });
});
