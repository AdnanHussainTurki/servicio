import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { CreateFlow } from "./CreateFlow";
import { useStore } from "../store";
import type { WorkerStatus } from "../types";

vi.mock("../api", () => ({
  api: {
    detectWorkers: vi.fn().mockResolvedValue([
      { label: "Custom worker", source: "generic", name: "", command: "", args: [], working_dir: "/p", run_mode: { type: "daemon", concurrency: 1 } },
    ]),
    addWorker: vi.fn().mockResolvedValue(undefined),
  },
}));

function seedWorkers(list: WorkerStatus[]) {
  useStore.getState().setWorkers(list);
}

beforeEach(() => {
  useStore.getState().reset();
});

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
    const nameInput = screen.getByLabelText(/^name/i) as HTMLInputElement;
    expect(nameInput.value).toBe("q");
    expect(nameInput.readOnly).toBe(true);
  });

  it("edit mode prefills the Display name field", () => {
    render(
      <CreateFlow
        onDone={() => {}}
        onCancel={() => {}}
        editWorker={{
          name: "q",
          display_name: "My Queue",
          command: "php",
          args: [],
          working_dir: "/srv",
          env: {},
          run_mode: { type: "daemon", concurrency: 1 },
          restart: { kind: "on_failure", max_retries: 5, base_secs: 1, max_secs: 60, reset_window_secs: 30 },
          autostart: true,
          enabled: true,
          group: null,
          tags: [],
        }}
      />,
    );

    const display = screen.getByLabelText(/display name/i) as HTMLInputElement;
    expect(display.value).toBe("My Queue");
    // The locked identity field remains the raw name.
    expect((screen.getByLabelText(/^name/i) as HTMLInputElement).value).toBe("q");
  });

  it("offers existing groups as a datalist and existing tags as clickable chips", async () => {
    seedWorkers([
      {
        name: "w1",
        run_mode: { type: "daemon", concurrency: 1 },
        instances: [],
        group: "billing",
        tags: ["redis"],
      },
    ]);

    const { container } = render(<CreateFlow onDone={() => {}} onCancel={() => {}} />);

    // Detect → scan → start from scratch → Command, then Command → Mode → Recovery.
    fireEvent.click(screen.getByText(/scan/i));
    fireEvent.click(await screen.findByText(/start from scratch/i));
    fireEvent.change(screen.getByLabelText(/^command$/i), { target: { value: "php" } });
    fireEvent.click(screen.getByText(/next/i));
    fireEvent.click(screen.getByText(/next/i));

    // Group field is a datalist-backed combobox containing the existing group.
    const groupInput = screen.getByLabelText(/group/i) as HTMLInputElement;
    expect(groupInput.getAttribute("list")).toBe("cf-groups");
    const datalist = container.querySelector("#cf-groups");
    expect(datalist?.querySelector('option[value="billing"]')).not.toBeNull();

    // Existing tag is rendered as a clickable suggestion chip; clicking appends it.
    const chip = screen.getByLabelText(/add tag redis/i);
    expect(chip).toBeDefined();
    fireEvent.click(chip);
    expect((screen.getByLabelText(/tags/i) as HTMLInputElement).value).toContain("redis");
  });
});
