import { describe, it, expect, beforeEach, vi } from "vitest";
import { render } from "@testing-library/react";
import { useStore } from "../store";
import { WorkerDetail } from "./WorkerDetail";

vi.mock("../api", () => ({
  api: { startWorker: vi.fn(), stopWorker: vi.fn(), restartWorker: vi.fn(), metrics: vi.fn().mockResolvedValue([]) },
  withError: (p: unknown) => p,
}));

describe("WorkerDetail", () => {
  beforeEach(() => useStore.getState().reset());

  // Regression: opening a worker with no logs/metrics yet must NOT infinite-loop.
  // (A `?? []` inside a Zustand selector returns a new array each render → React 19
  // "Maximum update depth exceeded" → blank screen.)
  it("renders without an infinite loop when the worker has no logs or metrics", () => {
    useStore.getState().setWorkers([
      {
        name: "fresh",
        run_mode: { type: "daemon", concurrency: 1 },
        instances: [{ index: 0, state: "running", restart_count: 0, pid: 123 }],
      },
    ]);
    expect(() => render(<WorkerDetail name="fresh" onBack={() => {}} />)).not.toThrow();
  });

  // Regression: scheduled (idle) + batch (completed) run-modes must render, not crash.
  it("renders a scheduled worker whose instance is idle", () => {
    useStore.getState().setWorkers([
      {
        name: "cron",
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        run_mode: { type: "scheduled", schedule: { cron: "0 3 * * *" }, overlap: "skip" } as any,
        instances: [{ index: 0, state: "idle", restart_count: 0, pid: null }],
      },
    ]);
    expect(() => render(<WorkerDetail name="cron" onBack={() => {}} />)).not.toThrow();
  });
});
