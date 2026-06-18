import { describe, it, expect, beforeEach } from "vitest";
import { useStore } from "./store";

describe("store", () => {
  beforeEach(() => useStore.getState().reset());

  it("seeds workers from list", () => {
    useStore.getState().setWorkers([
      { name: "q", run_mode: { type: "daemon", concurrency: 1 }, instances: [] },
    ]);
    expect(Object.keys(useStore.getState().workers)).toEqual(["q"]);
  });

  it("applies a state event to the matching instance", () => {
    useStore.getState().setWorkers([
      { name: "q", run_mode: { type: "daemon", concurrency: 1 },
        instances: [{ index: 0, state: "starting", restart_count: 0, pid: null }] },
    ]);
    useStore.getState().applyEvent({ kind: "state", worker: "q", instance: 0, from: "starting", to: "running" });
    expect(useStore.getState().workers["q"].instances[0].state).toBe("running");
  });

  it("appends log lines with a ring-buffer cap", () => {
    const s = useStore.getState();
    for (let i = 0; i < 1100; i++) {
      s.applyEvent({ kind: "log", worker: "q", instance: 0, stream: "stdout", line: `l${i}` });
    }
    const logs = useStore.getState().logs["q"];
    expect(logs.length).toBe(1000);
    expect(logs[logs.length - 1]).toContain("l1099");
  });

  it("buffers metric samples per worker with a cap", () => {
    for (let i = 0; i < 250; i++) {
      useStore.getState().applyEvent({ kind: "metric", worker: "q", instance: 0, ts: i, cpu: 1.0, mem: 100 });
    }
    const m = useStore.getState().metrics["q"];
    expect(m.length).toBe(200);
    expect(m[m.length - 1].ts).toBe(249);
  });

  it("sets and clears the last error", () => {
    useStore.getState().setError("boom");
    expect(useStore.getState().lastError).toBe("boom");
    useStore.getState().setError(null);
    expect(useStore.getState().lastError).toBeNull();
  });
});
