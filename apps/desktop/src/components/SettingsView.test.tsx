import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { SettingsView } from "./SettingsView";

const installService = vi.fn().mockResolvedValue(undefined);
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
});
