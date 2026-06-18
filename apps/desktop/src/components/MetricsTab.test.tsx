import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { useStore } from "../store";
import { MetricsTab } from "./MetricsTab";

vi.mock("../api", () => ({ api: { metrics: vi.fn().mockResolvedValue([]) } }));

describe("MetricsTab", () => {
  beforeEach(() => useStore.getState().reset());
  it("shows current cpu/mem from the latest sample", () => {
    useStore.getState().applyEvent({ kind: "metric", worker: "q", instance: 0, ts: 1, cpu: 4.2, mem: 1048576 });
    render(<MetricsTab worker="q" />);
    expect(screen.getByText(/4\.2/)).toBeDefined();        // cpu %
    expect(screen.getByText(/1(\.0)? ?MB/i)).toBeDefined();  // mem formatted (1 MiB → "1.0 MB" or "1 MB")
  });
});
