# Servicio Phase 3.2 — GUI Service Integration

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. New visual components built with `frontend-design` (match the 2b/2c control-room aesthetic). Checkbox steps.

**Goal:** A Settings panel to install/uninstall the login service + show its status, and have the GUI prefer an already-running daemon (the service) over spawning its own sidecar.

**Architecture:** Tauri bridge commands shell the bundled `servicio-daemon` binary's `install-service`/`uninstall-service`/`service-status` subcommands (reuse 2b's daemon-path resolution). The Settings view replaces the 2b "coming soon" stub: a "Start on login" toggle + status line + the theme toggle. `ensure_daemon` already connects-before-spawning, so a running service is used automatically; we surface which is in use.

**Tech Stack:** Tauri 2.11 (Rust shell-out via `std::process::Command`), React/TS/Vitest. No new deps.

**Builds on:** 3.1 (merged). Spec §4.

---

## Task 1: bridge service commands (build-verified)
**Files:** `apps/desktop/src-tauri/src/{sidecar,bridge,main}.rs`

Service install is daemon-local (shells the daemon binary), so the bridge runs the binary's subcommands rather than going over the socket.

- [ ] **Step 1 — resolve + run helper.** In `apps/desktop/src-tauri/src/sidecar.rs`, add a helper that resolves the daemon binary path (reuse the existing `current_exe sibling → "servicio-daemon"` logic from `ensure_daemon`; factor it into `pub fn daemon_program() -> String`) and runs a subcommand:
```rust
pub fn daemon_program() -> String {
    std::env::current_exe().ok()
        .and_then(|p| p.parent().map(|d| d.join("servicio-daemon")))
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "servicio-daemon".to_string())
}

/// Run a daemon subcommand, capturing stdout. Used for service-status/install/uninstall.
pub fn run_daemon_subcommand(args: &[&str]) -> std::io::Result<String> {
    let out = std::process::Command::new(daemon_program()).args(args).output()?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
```
Update `ensure_daemon` to use `daemon_program()` if it has its own inline copy (DRY) — optional, but make `daemon_program` the single source.
- [ ] **Step 2 — bridge fns.** In `bridge.rs`:
```rust
use crate::sidecar::run_daemon_subcommand;

pub fn service_status() -> Result<serde_json::Value, String> {
    let out = run_daemon_subcommand(&["service-status"]).map_err(|e| e.to_string())?;
    serde_json::from_str(out.trim()).map_err(|e| format!("parse service-status: {e} (got: {out})"))
}

pub fn install_service() -> Result<(), String> {
    run_daemon_subcommand(&["install-service"]).map(|_| ()).map_err(|e| e.to_string())
}

pub fn uninstall_service() -> Result<(), String> {
    run_daemon_subcommand(&["uninstall-service"]).map(|_| ()).map_err(|e| e.to_string())
}
```
- [ ] **Step 3 — tauri commands.** In `main.rs`:
```rust
#[tauri::command]
fn service_status() -> Result<serde_json::Value, String> { bridge::service_status() }
#[tauri::command]
fn install_service() -> Result<(), String> { bridge::install_service() }
#[tauri::command]
fn uninstall_service() -> Result<(), String> { bridge::uninstall_service() }
```
Add `service_status, install_service, uninstall_service` to `generate_handler![...]`. (These are sync `fn` — Tauri allows non-async commands.)
- [ ] **Step 4 — verify.** `cd apps/desktop/src-tauri && cargo build` → clean (the existing bridge_integration tests still pass; no new Rust test — these shell an external binary). `cargo test --test bridge_integration` green.
- [ ] **Step 5 — commit:** `git add apps/desktop/src-tauri && git commit -m "feat(gui): bridge service-status/install/uninstall (shell daemon subcommands)"`

---

## Task 2: Settings panel (frontend-design, TDD)
**Files:** `apps/desktop/src/components/SettingsView.tsx` (+ test), `src/api.ts`, `App.tsx`

INVOKE `frontend-design`; match the app.

- [ ] **Step 1 — api wrappers.** Add to `apps/desktop/src/api.ts` `api`:
```ts
  serviceStatus: () => invoke<{ installed: boolean; supported?: boolean }>("service_status"),
  installService: () => invoke<void>("install_service"),
  uninstallService: () => invoke<void>("uninstall_service"),
```
- [ ] **Step 2 — failing test.** Create `apps/desktop/src/components/SettingsView.test.tsx`:
```tsx
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
```
- [ ] **Step 3 — implement `SettingsView.tsx`.** A panel with:
  - On mount, `api.serviceStatus()` → state `{installed, supported}`.
  - A "Start on login" toggle (a `<label>` wrapping a checkbox, label text "Start on login") — checked = installed. On change: if turning on call `api.installService()` then refresh status; if off call `api.uninstallService()` then refresh. Guard errors (show inline message). If `supported === false`, disable the toggle + note "not supported on this OS".
  - A status line ("Service installed / not installed"), and a short explanation ("Runs the daemon at login so workers survive reboot").
  - frontend-design aesthetic (panel, signal accent, mono where apt).
- [ ] **Step 4 — wire into App.** The Sidebar already has a "Settings" nav item routing to a stub ("Settings coming soon"). Replace that stub render with `<SettingsView />`. Keep the theme toggle where it is (sidebar) or move into SettingsView — leaving it in the sidebar is fine.
- [ ] **Step 5 — verify.** `npm run test` (SettingsView + existing) + `npm run build` clean. `cd src-tauri && cargo build` clean.
- [ ] **Step 6 — commit:** `git add apps/desktop/src && git commit -m "feat(gui): settings panel — start-on-login service toggle (frontend-design)"`

---

## Task 3: verify render + finalize
- [ ] **Step 1 — full verify.** Root `cargo test` (engine unaffected), `cd apps/desktop && npm run test && npm run build`, `cd src-tauri && cargo build`.
- [ ] **Step 2 — render check.** Start `npm run dev`; if a Playwright tool is available, navigate, click Settings, confirm the panel + toggle render (no white screen, 0 console errors beyond known warnings); screenshot. Document.
- [ ] **Step 3 — commit (if any doc/tweak).** Otherwise nothing.

---

## Definition of Done (3.2)
- Bridge `service_status`/`install_service`/`uninstall_service` commands shell the daemon binary.
- Settings panel: start-on-login toggle (install/uninstall), status display, OS-unsupported handling.
- GUI uses a running service automatically (ensure_daemon connect-first — already true).
- Frontend + Rust build clean; Vitest green; render verified.

## Out of scope
- Universal build + updater (3.3). Showing service-vs-sidecar provenance in detail (status line is enough).

## Self-review notes
- Spec §4 covered. Bridge shells the daemon subcommands (daemon-local op). `daemon_program()` DRY-factored. Settings toggle install/uninstall + status; OS-unsupported guarded. frontend-design for the panel. Types: `api.{serviceStatus,installService,uninstallService}`; bridge `service_status`/`install_service`/`uninstall_service`; `run_daemon_subcommand`/`daemon_program`.
