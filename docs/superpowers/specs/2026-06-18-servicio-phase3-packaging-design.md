# Servicio Phase 3 — Packaging & Service Install Design Spec

**Date:** 2026-06-18
**Status:** Approved (scope C); built autonomously via /loop.
**Builds on:** Phases 1, 2a, 2b, 2c (all merged — product functionally complete).
**Parent design:** `docs/superpowers/specs/2026-06-18-servicio-design.md`

## 1. Summary

Phase 3 makes Servicio installable + truly always-on: the daemon installs as an OS service
(launchd / systemd) that starts at login and survives reboot; the GUI gains a Settings
toggle to manage it and prefers the installed service over its own sidecar; and the build
gains a universal macOS binary + a fully-wired Tauri auto-updater. Apple code-signing /
notarization is wired as config but stays blocked until the user supplies a Developer cert.

### Goals
- `servicio-daemon install-service` / `uninstall-service` / `service-status`: install the
  daemon as a launchd LaunchAgent (macOS) or systemd user unit (Linux), auto-start at login.
- GUI Settings: "Start on login" toggle + service status; GUI connects to the service if
  present instead of spawning a sidecar.
- Universal macOS app + sidecar (aarch64 + x86_64); Tauri auto-updater (own signing key) +
  config; Apple signing/notarization config + docs (blocked on cert).

### Non-goals
- Windows Service (trait-stubbed; impl later — dev host is macOS).
- System-level LaunchDaemon / pre-login headless (needs sudo; user-level LaunchAgent at
  login is the no-admin default).
- A hosted update server (updater is wired + signs artifacts; endpoint is config the user sets).
- Secrets/keychain, audit-view (carried deferrals, separate).

## 2. Decisions

| Topic | Decision |
|---|---|
| Scope | C — service install + universal build + updater (this phase). |
| macOS service | user LaunchAgent (`~/Library/LaunchAgents`), no sudo, starts at login. |
| Linux service | systemd **user** unit (`~/.config/systemd/user`), `systemctl --user`. |
| Windows | trait-stubbed, returns "unsupported" for now. |
| Service command | runs `<daemon-exe> serve --base <default base>`; reconciles autostart workers. |
| GUI vs service | if a daemon is already reachable (service running), connect; else sidecar (2b behaviour). |
| Updater | Tauri updater plugin + `tauri signer` keypair (NOT Apple-gated — fully wired). |
| Apple signing | config keys + docs; identity left empty (unsigned) until user provides cert. |
| Delivery | three sub-plans: 3.1 service install, 3.2 GUI integration, 3.3 universal+updater. |

## 3. Sub-plan 3.1 — OS-service install (engine/daemon)

New `crates/servicio-daemon/src/service.rs`:
- **Pure generators (unit-tested):**
  - `launchd_plist(label, program, args, base) -> String` — a LaunchAgent plist with
    `RunAtLoad=true`, `KeepAlive=true`, `ProgramArguments=[exe, "serve", "--base", base]`.
  - `systemd_unit(exe, base) -> String` — `[Service] ExecStart=…serve --base …`,
    `Restart=always`, `[Install] WantedBy=default.target`.
- **Install/uninstall/status** behind a small `ServiceManager` with `#[cfg(target_os)]` impls:
  - `install(spec, load: bool)` writes the unit file to the platform dir (parameterized so
    tests use a tempdir) and, when `load`, runs `launchctl load -w <plist>` /
    `systemctl --user enable --now servicio`.
  - `uninstall(load: bool)` unloads + removes the file.
  - `status() -> { installed: bool, running: bool }` — file exists + `launchctl list` /
    `systemctl --user is-active` (best-effort; running is informational).
- Service runs `std::env::current_exe()` → `serve --base <Paths::default_base()>`.
- **CLI:** `servicio-daemon install-service [--base <dir>]`, `uninstall-service`,
  `service-status` (prints installed/running).
- **Testability:** plist/unit string generators + file-write to an injected dir are unit-
  tested with tempdirs; the actual `launchctl`/`systemctl` invocation is gated by the `load`
  flag (false in tests) so CI never installs a real service.

## 4. Sub-plan 3.2 — GUI service integration

- **Bridge:** `install_service` / `uninstall_service` / `service_status` Tauri commands that
  shell the daemon binary's new subcommands (resolve the bundled `servicio-daemon` path as in
  2b's sidecar resolution) and return status.
- **Sidecar vs service:** on startup, `ensure_daemon` already tries to connect before
  spawning — so if the service is running, the GUI connects to it and does NOT spawn a second
  daemon (single-instance lock also guards this). Add: if a service is installed+running,
  never spawn the sidecar; surface which one is in use.
- **Settings view:** replace the 2b "Settings coming soon" stub with a real panel:
  "Start Servicio on login" toggle (calls install/uninstall_service), a status line
  (service installed? running? daemon connected?), and the existing theme toggle could move
  here. Built with frontend-design, matching the aesthetic.

## 5. Sub-plan 3.3 — Universal build + updater

- **Universal binary:** `rustup target add x86_64-apple-darwin`; build the daemon for both
  arches + `lipo` into a universal sidecar; `tauri build --target universal-apple-darwin`
  for a universal `.app`/`.dmg`. Update `prepare-sidecar.sh` to emit both-arch sidecars.
- **Updater (fully wired, not Apple-gated):**
  - `tauri signer generate` → an updater keypair; public key into `tauri.conf.json`
    `plugins.updater.pubkey`; private key documented as a build secret (env var).
  - Add `@tauri-apps/plugin-updater` + `tauri-plugin-updater`; config an `endpoints` URL
    (placeholder the user sets) + a `latest.json` manifest format note.
  - GUI: a minimal "check for updates" action (Settings) using the updater API; guarded so
    it no-ops without an endpoint.
- **Apple signing (blocked, documented):** `tauri.conf.json` `bundle.macOS`
  `signingIdentity` + `entitlements` + notarization env vars wired but left empty; a
  `docs/RELEASING.md` explains exactly what the user runs once they have a Developer cert
  (`codesign`, `xcrun notarytool`, or Tauri's built-in signing env vars).

## 6. Testing

- **3.1:** unit-test `launchd_plist`/`systemd_unit` content (label, RunAtLoad/KeepAlive,
  ExecStart, base path); install/uninstall against a tempdir with `load=false` asserting the
  file is written/removed; `service-status` shape. No real service is loaded in tests.
- **3.2:** Rust bridge integration for the service commands (status against a tempdir);
  Vitest for the Settings panel (toggle calls the right command, status rendering). Render-
  verify via Playwright.
- **3.3:** build-level — confirm universal build produces a fat binary (`lipo -info`),
  updater config parses, `tauri build` succeeds; updater signing key generates. Document the
  manual signed-release steps. (Actual notarization needs the cert — manual.)
- Engine `cargo test` stays green; GUI tests green; builds clean.

## 7. Ship-ready definition
The app is "ready to ship" (for unsigned/internal distribution) when: the daemon can install
as a login service (3.1) and the GUI manages it (3.2); a universal `.dmg` builds with the
updater wired (3.3); `RELEASING.md` documents the one remaining user-gated step (Apple
signing/notarization). Signed public distribution remains blocked on the user's Developer cert.

## 8. Open questions / future
- Windows Service impl; system-level LaunchDaemon option for pre-login headless.
- Hosted update endpoint + CI release pipeline.
- Carried deferrals: tokio-free `servicio-types`; constant-time token compare; scheduled
  overlap Queue/KillPrevious; folder-picker dialog; GUI reconnect/backoff.
