import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { useStore } from "../store";
import { GroupsView } from "./GroupsView";

vi.mock("../api", () => ({ api: { startWorker: vi.fn(), stopWorker: vi.fn() } }));

describe("GroupsView", () => {
  beforeEach(() => useStore.getState().reset());
  it("shows a folder card per group and drills into one", () => {
    useStore.getState().setWorkers([
      { name: "queue", group: "billing", tags: [], run_mode: { type: "daemon", concurrency: 1 } as any, instances: [{ index: 0, state: "running", restart_count: 0, pid: 1 }] },
      { name: "cron", group: "billing", tags: [], run_mode: { type: "daemon", concurrency: 1 } as any, instances: [{ index: 0, state: "stopped", restart_count: 0, pid: null }] },
      { name: "loose", group: null, tags: [], run_mode: { type: "daemon", concurrency: 1 } as any, instances: [{ index: 0, state: "running", restart_count: 0, pid: 2 }] },
    ]);
    render(<GroupsView onOpenWorker={() => {}} onAddWorker={() => {}} />);
    expect(screen.getByText("billing")).toBeDefined();
    expect(screen.getByText(/ungrouped/i)).toBeDefined();
    // drill into billing
    fireEvent.click(screen.getByText("billing"));
    expect(screen.getByText("queue")).toBeDefined();
    expect(screen.getByText("cron")).toBeDefined();
    expect(screen.queryByText("loose")).toBeNull(); // not in this group
  });
});
