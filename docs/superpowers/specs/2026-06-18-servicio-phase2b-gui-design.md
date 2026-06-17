# Servicio Phase 2b — Minimal Tauri GUI Design Spec

**Date:** 2026-06-18
**Status:** Approved (design); implementation plan pending
**Builds on:** Phase 1 (engine + daemon) + Phase 2a (IPC socket API + `servicio-cli` Client), both merged to main.
**Parent design:** `docs/superpowers/specs/2026-06-18-servicio-design.md`

## 1. Summary

Phase 2b is the first graphical app: a Tauri desktop client that auto-starts the daemon
(as a bundled sidecar), connects over the Phase-2a socket API, and gives a friendly,
live-updating UI to see workers, watch their logs, start/stop/restart them, and add new
ones with a simple form. It is the "minimal GUI" — the full creation wizard, metrics
graphs, autodetect, and notifications are explicitly Phase 2c+.

### Goals
- Tauri app that bundles + auto-spawns `servicio-daemon serve` and connects to it.
- Card-grid dashboard of workers with live status (the approved visual design).
- Worker detail view with a live log stream and start/stop/restart controls.
- A simple add-worker form (daemon run-mode).
- All driven through a thin Rust bridge that reuses `servicio-cli`'s `Client` — no new
  protocol code.

### Non-goals (2b)
- 4-step creation wizard, framework autodetect UI (Phase 2c).
- Metrics graphs, native notifications, Events/audit tab (Phase 2c).
- Scheduled/batch run-mode UI (those run-modes don't exist in the engine yet).
- OS-service install, code signing/notarization, installer polish (packaging phase).
- Daemon surviving GUI close (deferred; 2b may stop the sidecar on exit).
- Full E2E browser-automation tests.

## 2. Decisions (from brainstorming)

| Topic | Decision |
|---|---|
| Scope | Minimal GUI now; wizard/metrics/notifications = Phase 2c. |
| Daemon launch | GUI spawns `servicio-daemon` as a Tauri **sidecar** on startup. |
| Bridge | React never touches the socket; Tauri **Rust backend** holds `servicio-cli::Client`. |
| Frontend | React + TypeScript + Vite + Tailwind + Zustand; dark/light. |
| Live updates | Backend `subscribe`s, forwards events to React via Tauri's event system. |
| Add UX | Simple form (not the wizard) for 2b. |
| Tests | Rust bridge ↔ real daemon over socket; React components with Vitest + mocked Tauri. |

## 3. Architecture

```
┌─ Tauri app ───────────────────────────────────────────┐
│  React frontend (TS)          Tauri Rust backend       │
│  ┌─────────────────┐  invoke  ┌──────────────────────┐ │
│  │ dashboard/detail│─────────►│ #[tauri::command]    │ │
│  │ add form        │◄─events──│  ps/start/stop/add…  │ │
│  └─────────────────┘          │  Client (socket)     │ │
│                               │  + sidecar mgmt      │ │
│                               └──────────┬───────────┘ │
└──────────────────────────────────────────┼────────────┘
                                socket (2a) │
                                  ┌──────────▼──────────┐
                                  │ servicio-daemon     │ (sidecar)
                                  └─────────────────────┘
```

- The browser sandbox cannot open a Unix socket, so the **Tauri Rust backend is the only
  thing that talks to the daemon.** It reuses `servicio-cli`'s `Client` (from 2a) — no
  duplicated protocol logic.
- **Command connection:** one `Client` held in Tauri-managed state (behind a `tokio::Mutex`)
  serves request/response commands.
- **Event connection:** a second `Client` calls `subscribe{topics:["state","log"]}`; a
  background task reads event frames and re-emits them to the frontend with
  `app_handle.emit("worker-event", payload)`. React subscribes via `@tauri-apps/api/event`.
- **Sidecar lifecycle:** on startup the backend resolves a base dir, checks for a live
  daemon (try connect); if absent, spawns the bundled `servicio-daemon serve --base <dir>`
  via Tauri's sidecar/`Command`, waits for the socket (poll with timeout), then connects.
  On app exit, the sidecar is stopped (survive-close is deferred). The token is read from
  `<base>/token` after the daemon writes it.

### Project layout
```
apps/
  desktop/
    package.json            # React app (vite, tailwind, zustand, @tauri-apps/api)
    index.html
    src/                    # React/TS
      main.tsx, App.tsx
      store.ts              # Zustand store (workers + log buffers)
      api.ts                # typed wrappers over tauri invoke + event listeners
      types.ts             # TS mirrors of ipc shapes
      components/          # Sidebar, WorkerCard, Dashboard, WorkerDetail, LogView, AddWorkerForm
    src-tauri/
      Cargo.toml            # Tauri crate; deps: servicio-cli (lib), servicio-ipc, tauri, tokio
      tauri.conf.json       # externalBin = servicio-daemon sidecar
      src/
        main.rs             # Tauri builder, manage state, register commands
        bridge.rs           # commands: daemon_status/list_workers/add_worker/start/stop/restart
        sidecar.rs          # spawn/connect/teardown daemon
        events.rs           # subscribe + forward to frontend
```

`apps/desktop/src-tauri` is its own Cargo crate. It is kept OUT of the root engine
workspace (a separate `[workspace]` or standalone) so Tauri's heavy build deps don't slow
`cargo test` on the engine crates. It depends on the engine crates by path.

## 4. Backend bridge (Rust)

Tauri commands (all async, return `Result<T, String>`; reuse `Client`):
| Command | Calls | Returns |
|---|---|---|
| `daemon_status` | `Client::daemon_info` (or connection check) | `{ connected, version, uptime_secs, worker_count, running_count }` |
| `list_workers` | `Client::list_workers` | `WorkerStatus[]` |
| `add_worker` | `Client::add_worker` | `()` |
| `start_worker` | `Client::start_worker` | `()` |
| `stop_worker` | `Client::stop_worker` | `()` |
| `restart_worker` | `stop_worker` then `start_worker` | `()` |

`restart_worker` is a backend convenience (the daemon has no restart RPC; compose
stop+start). `add_worker` builds a `WorkerSpec` from the form payload.

**Event forwarding:** `events.rs` runs a task that holds a subscribed `Client`, reads each
`Frame::Event`, and emits to the frontend:
- `worker-event` with `{ kind: "state", ... }` or `{ kind: "log", ... }` payloads.
The frontend has a single listener that dispatches into the store.

**Managed state:** `AppState { client: Mutex<Client>, base: PathBuf, token: String }`. The
event task uses its own `Client` (a separate connection) so a long-lived subscribe never
blocks command request/response.

## 5. Frontend (React/TS)

**Stack:** React + TS + Vite + Tailwind + Zustand. Dark/light theme via a class toggle.

**Screens:**
1. **App shell** — left sidebar (Dashboard / Logs / Settings) + daemon-status footer
   (connected ● / disconnected ○ with a "Start daemon" action that re-runs sidecar spawn).
2. **Dashboard** — card grid (approved layout): each `WorkerCard` shows name, run-mode,
   colored status dot (running=green / idle=grey / crashed=red), instance count, restart
   count, and inline Start/Stop. Top summary chips (running/crashed counts). "+ New worker".
3. **Worker detail** — header with state + Start/Stop/Restart; **Logs tab** = live stream
   (follow toggle, clear, copy); **Config tab** = read-only worker fields.
4. **Add worker form** — name, command, args (list), working_dir (Tauri folder dialog),
   concurrency, restart policy (kind + max_retries), autostart toggle. Submits `add_worker`,
   then refreshes the list.

**State (Zustand):** `{ workers: Record<name, WorkerStatus>, logs: Record<name, string[]>,
daemon: DaemonStatus }`. Seed with `list_workers` on mount; apply `worker-event`s — `state`
events update the matching instance's status; `log` events append to `logs[worker]` (capped
ring buffer, e.g. last 1000 lines). Start/stop are optimistic, reconciled by events.

**Types:** `types.ts` mirrors the ipc JSON: `WorkerStatus`, `InstanceStatus`, `StateEvent`,
`LogEvent`, `DaemonStatus`, plus the `AddWorker` form payload.

## 6. Testing

- **Rust bridge:** integration tests spawn the daemon in-process via `servicio-daemon`'s
  `serve()` (as in 2a) on a temp base dir, then drive the bridge command functions directly
  (factor command bodies so they take an injected `Client`/base, callable without a live
  Tauri runtime). Assert add→list, start→stop, restart, and event forwarding shape. Sidecar
  binary-spawn is exercised by a "manage=false" path in tests (inject an already-running
  daemon) so tests don't shell out to a bundled binary.
- **Frontend:** Vitest + Testing Library with `@tauri-apps/api` mocked. Cover: dashboard
  renders cards from store; status-color logic; log append + ring-buffer cap; add-form
  validation + the exact `add_worker` payload; event-listener dispatch updates the store.
- **Manual smoke (documented):** `npm run tauri dev` → daemon auto-starts → add a ticker
  worker → see it running on the dashboard → open detail → watch live logs → stop it.
- Full E2E (tauri-driver/Playwright) is out of scope for 2b.

## 7. Build

- `apps/desktop/src-tauri` = standalone Tauri Cargo crate depending on engine crates by
  path; kept out of the root workspace so engine `cargo test` stays fast.
- `servicio-daemon` declared as a Tauri **sidecar** (`tauri.conf.json` `bundle.externalBin`),
  built before the Tauri bundle.
- Dev loop: `npm run tauri dev`. Bundling to `.dmg`/`.msi`/`.AppImage` works via the Tauri
  bundler, but **signing/notarization + installer polish are the later packaging phase.**

## 8. Build phases (internal sequence)
1. Scaffold Tauri app (`apps/desktop`) + React/Vite/Tailwind/Zustand; blank window runs.
2. Backend: sidecar spawn + connect + `daemon_status` command; status footer in UI.
3. Backend commands: `list_workers`/`start`/`stop`/`restart`/`add_worker` + Rust tests.
4. Event forwarding: subscribe task → `worker-event` emit; frontend listener → store.
5. Dashboard card grid (live) + summary chips.
6. Worker detail + live Logs tab + Config tab.
7. Add-worker form (folder dialog) + validation.
8. Theming/polish; frontend component tests; README + manual smoke.

## 9. Open questions / future (Phase 2c+)
- 4-step creation wizard + framework autodetect UI.
- Metrics graphs (need engine metrics sampling first) + Events/audit tab + notifications.
- Daemon survive-GUI-close + OS-service install + run-on-boot.
- Reconnect/backoff UX when the daemon restarts under the GUI.
- Extract tokio-free `servicio-types` leaf crate (carried over from 2a) — not required for
  2b since the frontend consumes JSON, but still worth doing before broader reuse.
