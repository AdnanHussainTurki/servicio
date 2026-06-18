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

  it("edit mode starts on Command, prefills + locks the name", () => {
    render(
      <CreateFlow
        onDone={() => {}}
        onCancel={() => {}}
        editWorker={{
          name: "q",
          command: "php",
          args: ["artisan", "queue:work"],
          working_dir: "/srv",
          env: {},
          run_mode: { type: "daemon", concurrency: 3 },
          restart: { kind: "on_failure", max_retries: 5, base_secs: 1, max_secs: 60, reset_window_secs: 30 },
          autostart: true,
          enabled: true,
          group: "app",
          tags: ["redis"],
        }}
      />,
    );

    // Header reflects edit mode.
    expect(screen.getByText("Edit worker")).toBeDefined();

    // Lands directly on Command — no Detect/Scan UI nor Folder field.
    expect(screen.queryByText(/scan/i)).toBeNull();
    expect(screen.queryByLabelText(/folder/i)).toBeNull();

    // Command is prefilled.
    expect((screen.getByLabelText(/^command$/i) as HTMLInputElement).value).toBe("php");

    // Name is prefilled and read-only.
    const nameInput = screen.getByLabelText(/name/i) as HTMLInputElement;
    expect(nameInput.value).toBe("q");
    expect(nameInput.readOnly).toBe(true);
  });
});
