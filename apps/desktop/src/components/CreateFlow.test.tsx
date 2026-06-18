import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { CreateFlow } from "./CreateFlow";

vi.mock("../api", () => ({
  api: {
    detectWorkers: vi.fn().mockResolvedValue([
      { label: "Custom worker", source: "generic", name: "", command: "", args: [], working_dir: "/p", run_mode: { type: "daemon", concurrency: 1 } },
    ]),
    addWorker: vi.fn().mockResolvedValue(undefined),
  },
}));

describe("CreateFlow", () => {
  it("scans a path and lists suggestions", async () => {
    render(<CreateFlow onDone={() => {}} onCancel={() => {}} />);
    fireEvent.change(screen.getByLabelText(/folder/i), { target: { value: "/p" } });
    fireEvent.click(screen.getByText(/scan/i));
    expect(await screen.findByText(/custom worker/i)).toBeDefined();
  });
});
