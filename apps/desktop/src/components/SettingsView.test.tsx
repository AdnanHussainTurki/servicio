import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { SettingsView } from "./SettingsView";

const installService = vi.fn().mockResolvedValue(undefined);
vi.mock("../api", () => ({
  api: {
    serviceStatus: vi.fn().mockResolvedValue({ installed: false }),
    installService: (...a: unknown[]) => installService(...a),
    uninstallService: vi.fn().mockResolvedValue(undefined),
  },
}));

describe("SettingsView", () => {
  it("shows the start-on-login control and installs on toggle", async () => {
    render(<SettingsView />);
    const toggle = await screen.findByLabelText(/start on login/i);
    fireEvent.click(toggle);
    await waitFor(() => expect(installService).toHaveBeenCalled());
  });
});
