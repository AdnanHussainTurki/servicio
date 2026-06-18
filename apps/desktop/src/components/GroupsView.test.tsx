import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { useStore } from "../store";
import { GroupsView } from "./GroupsView";

const { startGroup, stopGroup } = vi.hoisted(() => ({
  startGroup: vi.fn(() => Promise.resolve({ started: 0 })),
  stopGroup: vi.fn(() => Promise.resolve({ stopped: 0 })),
}));

vi.mock("../api", () => ({
  api: { startWorker: vi.fn(), stopWorker: vi.fn(), startGroup, stopGroup },
  withError: (p: Promise<unknown>) => p,
}));

const MB = 1048576;

function seed() {
  useStore.getState().setWorkers([
    { name: "queue", group: "billing", tags: [], run_mode: { type: "daemon", concurrency: 1 }, instances: [{ index: 0, state: "running", restart_count: 0, pid: 1 }] },
    { name: "cron", group: "billing", tags: [], run_mode: { type: "daemon", concurrency: 1 }, instances: [{ index: 0, state: "running", restart_count: 0, pid: 3 }] },
    { name: "loose", group: null, tags: [], run_mode: { type: "daemon", concurrency: 1 }, instances: [{ index: 0, state: "running", restart_count: 0, pid: 2 }] },
  ]);
  // billing → 100 MB + 200 MB = 300 MB; loose → 25 MB
  useStore.getState().applyEvent({ kind: "metric", worker: "queue", instance: 0, ts: 1, cpu: 1, mem: 100 * MB });
  useStore.getState().applyEvent({ kind: "metric", worker: "cron", instance: 0, ts: 1, cpu: 2, mem: 200 * MB });
  useStore.getState().applyEvent({ kind: "metric", worker: "loose", instance: 0, ts: 1, cpu: 1, mem: 25 * MB });
}

describe("GroupsView", () => {
  beforeEach(() => {
    useStore.getState().reset();
    startGroup.mockClear();
    stopGroup.mockClear();
  });

  it("shows a folder card per group and drills into one", () => {
    useStore.getState().setWorkers([
      { name: "queue", group: "billing", tags: [], run_mode: { type: "daemon", concurrency: 1 }, instances: [{ index: 0, state: "running", restart_count: 0, pid: 1 }] },
      { name: "cron", group: "billing", tags: [], run_mode: { type: "daemon", concurrency: 1 }, instances: [{ index: 0, state: "stopped", restart_count: 0, pid: null }] },
      { name: "loose", group: null, tags: [], run_mode: { type: "daemon", concurrency: 1 }, instances: [{ index: 0, state: "running", restart_count: 0, pid: 2 }] },
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

  it("shows aggregate group memory on the folder card", () => {
    seed();
    render(<GroupsView onOpenWorker={() => {}} onAddWorker={() => {}} />);
    // billing total memory = 300 MB
    expect(screen.getByText("300 MB")).toBeDefined();
  });

  it("sorts groups by memory descending with Ungrouped last", () => {
    seed();
    render(<GroupsView onOpenWorker={() => {}} onAddWorker={() => {}} />);
    const headings = screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent);
    expect(headings).toEqual(["billing", "Ungrouped"]);
  });

  it("Start all calls api.startGroup with the group name without drilling in", () => {
    seed();
    render(<GroupsView onOpenWorker={() => {}} onAddWorker={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Start all in billing" }));
    expect(startGroup).toHaveBeenCalledWith("billing");
    // still on the folder grid (no drill-in): the other folder is still visible
    expect(screen.getByText(/ungrouped/i)).toBeDefined();
  });

  it("Stop all confirms before calling api.stopGroup", () => {
    seed();
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<GroupsView onOpenWorker={() => {}} onAddWorker={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Stop all in billing" }));
    expect(confirmSpy).toHaveBeenCalled();
    expect(stopGroup).not.toHaveBeenCalled(); // confirm denied → no-op

    confirmSpy.mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: "Stop all in billing" }));
    expect(stopGroup).toHaveBeenCalledWith("billing");
    confirmSpy.mockRestore();
  });
});
