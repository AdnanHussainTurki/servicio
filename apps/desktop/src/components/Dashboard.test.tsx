import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { useStore } from "../store";
import { Dashboard } from "./Dashboard";

vi.mock("../api", () => ({
  api: { startWorker: vi.fn(), stopWorker: vi.fn() },
}));

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
});
