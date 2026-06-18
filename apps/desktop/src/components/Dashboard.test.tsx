import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { useStore } from "../store";
import { Dashboard } from "./Dashboard";
import type { InstanceStatus } from "../types";

vi.mock("../api", () => ({
  api: { startWorker: vi.fn(), stopWorker: vi.fn() },
  withError: vi.fn(),
}));

function inst(): InstanceStatus[] {
  return [{ index: 0, state: "running", restart_count: 0, pid: 10 }];
}

describe("Dashboard", () => {
  beforeEach(() => useStore.getState().reset());

  it("renders a card per worker with its status", () => {
    useStore.getState().setWorkers([
      { name: "queue", run_mode: { type: "daemon", concurrency: 2 },
        instances: [{ index: 0, state: "running", restart_count: 0, pid: 10 },
                    { index: 1, state: "running", restart_count: 0, pid: 11 }] },
      { name: "img", run_mode: { type: "daemon", concurrency: 1 },
        instances: [{ index: 0, state: "crashed", restart_count: 5, pid: null }] },
    ]);
    render(<Dashboard onOpen={() => {}} onAdd={() => {}} />);
    expect(screen.getByText("queue")).toBeDefined();
    expect(screen.getByText("img")).toBeDefined();
    expect(screen.getByText(/crashed/i)).toBeDefined();
  });

  it("renders a section header for each distinct group plus Ungrouped", () => {
    useStore.getState().setWorkers([
      { name: "api", group: "shop", tags: ["web"],
        run_mode: { type: "daemon", concurrency: 1 }, instances: inst() },
      { name: "worker", group: "billing", tags: ["queue"],
        run_mode: { type: "daemon", concurrency: 1 }, instances: inst() },
      { name: "loner", group: null, tags: [],
        run_mode: { type: "daemon", concurrency: 1 }, instances: inst() },
    ]);
    render(<Dashboard onOpen={() => {}} onAdd={() => {}} />);
    expect(screen.getByText("shop")).toBeDefined();
    expect(screen.getByText("billing")).toBeDefined();
    expect(screen.getByText("Ungrouped")).toBeDefined();
  });

  it("clicking a tag filter chip narrows the visible worker cards", () => {
    useStore.getState().setWorkers([
      { name: "api", group: "shop", tags: ["web"],
        run_mode: { type: "daemon", concurrency: 1 }, instances: inst() },
      { name: "worker", group: "billing", tags: ["queue"],
        run_mode: { type: "daemon", concurrency: 1 }, instances: inst() },
    ]);
    render(<Dashboard onOpen={() => {}} onAdd={() => {}} />);
    expect(screen.getByText("api")).toBeDefined();
    expect(screen.getByText("worker")).toBeDefined();

    // the filter bar renders a "web" toggle chip; clicking it should hide "worker"
    fireEvent.click(screen.getByRole("button", { name: "web" }));
    expect(screen.getByText("api")).toBeDefined();
    expect(screen.queryByText("worker")).toBeNull();
  });
});
