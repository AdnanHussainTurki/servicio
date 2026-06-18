import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { SettingsView } from "./SettingsView";

const installService = vi.fn().mockResolvedValue(undefined);
const saveDialog = vi.fn().mockResolvedValue(null);
const openFileDialog = vi.fn().mockResolvedValue(null);
const exportWorkersTo = vi.fn().mockResolvedValue(0);
const importWorkersFrom = vi.fn().mockResolvedValue(0);
vi.mock("../api", () => ({
  api: {
    serviceStatus: vi.fn().mockResolvedValue({ installed: false }),
    installService: (...a: unknown[]) => installService(...a),
    uninstallService: vi.fn().mockResolvedValue(undefined),
    appVersion: vi.fn().mockResolvedValue("0.1.0"),
    daemonStatus: vi.fn().mockResolvedValue({
      connected: true,
      version: "9.9.9",
      uptime_secs: 3661,
      worker_count: 2,
      running_count: 1,
    }),
    daemonLog: vi.fn().mockResolvedValue({ log: "hello daemon" }),
    saveDialog: (...a: unknown[]) => saveDialog(...a),
    openFileDialog: (...a: unknown[]) => openFileDialog(...a),
    exportWorkersTo: (...a: unknown[]) => exportWorkersTo(...a),
    importWorkersFrom: (...a: unknown[]) => importWorkersFrom(...a),
  },
}));

describe("SettingsView", () => {
  it("shows the start-on-login control and installs on toggle", async () => {
    render(<SettingsView />);
    const toggle = await screen.findByLabelText(/start on login/i);
    fireEvent.click(toggle);
    await waitFor(() => expect(installService).toHaveBeenCalled());
  });

  it("renders the daemon log in the Debug viewer", async () => {
    render(<SettingsView />);
    await waitFor(() => expect(screen.getByText("hello daemon")).toBeInTheDocument());
  });

  it("shows diagnostics from daemonStatus", async () => {
    render(<SettingsView />);
    await waitFor(() => expect(screen.getByText("9.9.9")).toBeInTheDocument());
    expect(screen.getByText("connected")).toBeInTheDocument();
  });

  it("exports workers when the save dialog resolves a path", async () => {
    saveDialog.mockResolvedValueOnce("/tmp/x.json");
    exportWorkersTo.mockResolvedValueOnce(3);
    render(<SettingsView />);
    fireEvent.click(await screen.findByRole("button", { name: /export/i }));
    await waitFor(() => expect(exportWorkersTo).toHaveBeenCalledWith("/tmp/x.json"));
    await waitFor(() => expect(screen.getByText(/Exported 3 workers/)).toBeInTheDocument());
  });
});
