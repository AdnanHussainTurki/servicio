# Servicio — Design Spec

**Date:** 2026-06-18
**Status:** Approved (design); implementation plan pending

## 1. Summary

Servicio is a cross-platform desktop application (macOS, Windows, Linux) that runs and
supervises long-running and scheduled developer "service workers" — Laravel queue
workers, Python scripts, and any arbitrary command — in a local environment, keeping
them running reliably with minimal code intervention.

It pairs a friendly graphical interface with a robust background supervisor so workers
keep running even when the window is closed, the user logs out, or the machine reboots.

### Goals
- Keep arbitrary worker processes running "always," with automatic crash recovery.
- Support multiple run modes: always-on (daemon), scheduled (cron/interval), and batch
  (run N times).
- Configure concurrency, restart policy, scheduling, and run counts per worker.
- Capture, store, rotate, and live-stream logs for everything.
- Self-heal on crash/reboot; never burn CPU in a crash loop.
- Friendly, polished GUI; low/no code intervention required.
- Architect personal-use-first but ready to grow into a distributable product.

### Non-goals (v1)
- Remote control / managing the daemon from another machine (future).
- Bundling language runtimes (PHP/Python). Servicio supervises; it detects + warns if a
  runtime is missing.
- Container/VM orchestration (this is a process supervisor, not Docker).

## 2. Decisions (from brainstorming)

| Topic | Decision |
|---|---|
| Audience | Both: personal-first, architected to become a product. |
| Stack | Tauri GUI + React/TS frontend; standalone Rust daemon; Rust core engine. |
| Always-on | Yes — headless daemon survives GUI close + reboot; per-worker autostart override. |
| Run modes | All of: concurrency, scheduled, restart policy, run-to-completion count. |
| Worker definition | GUI wizard + importable YAML config + framework autodetect. |
| v1 scope | Full vision before "ship"; build sequenced in phases internally. |
| Platforms | macOS, Windows, Linux — all first-class. |
| Dashboard UI | Card grid, left sidebar nav. |
| Worker detail | Tabs: Logs / Metrics / Config / Events. |
| Create flow | 4-step wizard (Command → Mode → Recovery → Review) with autodetect hints. |

## 3. Architecture

Three cooperating processes with clean boundaries:

```
┌─────────────────┐   local IPC    ┌──────────────────────┐
│  GUI (Tauri)    │◄──────────────►│  servicio-daemon     │
│  React frontend │  (Unix socket  │  (Rust, OS service)  │
│  = thin client  │   / named pipe)│  = supervisor core   │
└─────────────────┘   + auth token └──────────┬───────────┘
                                               │ spawns / monitors
                                   ┌───────────┼───────────┐
                                   ▼           ▼           ▼
                              worker proc  worker proc  worker proc
```

- **servicio-daemon** — the product's hard core. Standalone Rust binary installed as an
  OS service (launchd / Windows Service / systemd). Survives GUI close, logout, reboot.
  Owns process lifecycle, run modes, restart/scheduling/health, log capture, and state.
  Exposes a local authenticated API. Source of truth.
- **GUI (Tauri app)** — separate binary, pure client. Talks to the daemon over local IPC.
  Renders state, sends commands. Closing it does not stop workers.
- **Workers** — the supervised OS processes.

Rationale: "always running" requires a process that is not the window. Mirrors Docker
daemon/Desktop, ollama, tailscaled. Enables a future thin CLI and the product path
(same daemon, optional remote control later) without rework.

### Cross-platform matrix

| Concern | macOS | Windows | Linux |
|---|---|---|---|
| GUI | Tauri (WebKit) | Tauri (WebView2) | Tauri (WebKitGTK) |
| Daemon as service | launchd `.plist` | Windows Service (SCM) | systemd unit (user/system) |
| IPC | Unix socket | named pipe | Unix socket |
| Run on boot | LaunchAgent | service auto-start | systemd `enable` |
| Process control | POSIX signals | Win32 job objects | POSIX signals |

Platform differences (signals, process groups, service install, IPC transport) hide
behind traits in the daemon; the rest is shared. Targets: Apple Silicon + Intel,
x64/ARM Windows, x64/ARM Linux.

## 4. Process Model & Run Modes

Core unit = **Worker** (a definition). The daemon spawns **instances** (OS processes).

### Worker definition fields
- `name`, `command` + `args`, `working_dir` (absolute; validated to exist before spawn)
- `env` vars (+ optional `.env` file load, resolved relative to `working_dir`)
- `runtime` hint (php / python / node / generic) — for detection + missing-runtime warning
- `run_mode` + mode config (below)
- `autostart` (start on daemon boot?), `enabled`
- `restart_policy`, `health` (optional), `log` config, `resource_limits` (optional, later)
- Clean inherited env by default + user vars on top (consistent GUI-launch vs boot-launch).

### Run modes
1. **Daemon (always-on)** — keep `concurrency: N` instances alive forever; restart policy
   applies (crash → respawn).
2. **Scheduled** — run on cron expression or interval. `overlap_policy`:
   skip / queue / kill-previous if the prior run is still going. Target for crontab import.
3. **Batch** — run to completion up to `run_count` times (1 = once), optional delay
   between runs; stops when done.

### Restart policy (daemon/batch)
`always | on-failure | never`, with `max_retries`, **exponential backoff** (e.g.
1s→2s→4s… capped at 60s), and `reset_window` (stable for X s → reset retry counter).

### Health check (optional, daemon mode)
process-alive (default) | HTTP ping | custom command exit-0, on an interval.
Unhealthy → restart. Catches hung-but-alive workers.

### Lifecycle
`stopped → starting → running → (stopping → stopped | crashed → backoff → starting)`.
Scheduled adds `idle ↔ running`; batch ends `completed` / `failed`. Graceful stop =
SIGTERM then SIGKILL after a timeout (Windows: job-object terminate). The state machine
lives in the daemon; one async (Tokio) supervisor task per running instance.

## 5. Logs, Monitoring & Error Recovery

### Logs
- Capture each instance's stdout+stderr line-by-line, tagged
  `(worker, instance, stream, timestamp)`. Owned by the daemon → never lost when GUI closed.
- Store: append to per-worker log files on disk + index metadata in SQLite.
- **Rotation**: size cap + max files + max age; auto-prune (prevents disk fill).
- **Live streaming**: GUI subscribes over IPC; daemon tails new lines in real time.
  Per-worker and merged "all workers" views; regex search/filter; level highlight
  (ERROR/WARN); pause/follow; copy/export. Ring buffer in daemon for instant backscroll.

### Monitoring / metrics
Per instance: state, uptime, restart count, last exit code, PID, CPU%, mem (sampled).
Per worker: aggregate + uptime %. Dashboard shows all workers at a glance. Optional
history graphs (cpu/mem/restarts) from sampled data.

### Error recovery (the "always running" guarantee)
1. **Crash** → restart policy + backoff.
2. **Crash-loop guard** → after `max_retries` in `reset_window`, mark `crashed`, stop
   retrying, alert. No CPU burn.
3. **Hung process** → health check fails → kill + restart.
4. **Daemon crash** → OS service manager restarts it (launchd/systemd/SCM KeepAlive). On
   restart, daemon reads SQLite and **reconciles** — restarts workers that should be
   running. Self-healing.
5. **Reboot** → service auto-starts → `autostart` workers return.
6. **Orphan cleanup** → daemon tracks PIDs; on restart adopts/reaps strays (no duplicates
   or zombies).

### Alerts & audit
- Native OS notification + in-app badge on crash, crash-loop, recovery.
  Optional later: webhook / email / Slack (product tier).
- **Audit trail**: daemon logs its own events (started X, restarted Y ×3, scheduled Z),
  separate from worker logs — answers "what happened at 3am."

## 6. Autodetect, Config Import & Data Model

### Config import/export
Canonical `servicio.yaml` (or `.json`) per project. GUI reads → creates workers and
writes back on edit. Version-controllable, team-shareable. Definitions are the source of
truth in the DB; the file is an import/export view, not live-linked unless the user
enables "sync to file."

```yaml
version: 1
workers:
  - name: laravel-queue
    command: php artisan queue:work --tries=3
    working_dir: ./
    runtime: php
    run_mode: { type: daemon, concurrency: 4 }
    restart: { policy: on-failure, max_retries: 5, backoff: exponential }
  - name: cleanup
    command: python scripts/cleanup.py
    run_mode: { type: scheduled, cron: "0 3 * * *" }
```

### Autodetect — pluggable detectors
Trait `Detector`; each framework is an isolated, additive unit. Point at a folder → daemon
scans → proposes workers → user confirms before anything runs (never auto-spawns):
- **Laravel** — `artisan` → suggest `queue:work`, `schedule:run` (every minute),
  `reverb`/`horizon` if present.
- **Python** — venv / `requirements.txt` / `*.py` → script workers, detect entrypoints.
- **Crontab import** — parse `crontab` / `cron.d` → scheduled workers.
- **Node** — `package.json` scripts, queue libs (e.g. BullMQ).
- **Generic / Procfile** — parse `Procfile` lines.
- Roadmap: more frameworks.

### Data model (SQLite, daemon-owned)
- `workers` — definition (§4 fields); JSON for mode/restart/health config.
- `instances` — runtime rows: worker_id, pid, state, started_at, exit_code, restart_count.
- `events` — audit trail (§5).
- `metrics` — sampled cpu/mem/uptime (rolling, pruned).
- `log_files` — index → on-disk log paths + rotation meta.
- `settings` — daemon config, IPC token, theme, etc.

## 7. GUI / UX

- **Shell** — left sidebar nav: Dashboard / Schedules / Logs / Detect / Settings;
  daemon-status footer.
- **Dashboard** — **card grid** of workers. Each card: name, run mode, colored status
  (running/idle/crashed), key metrics, quick error link. Top summary chips (N running /
  N crashed). "+ New worker" action.
- **Worker detail** — header with live state, Stop/Restart/Edit, and stat tiles (uptime,
  restarts, cpu, mem, instances). Tabs:
  - **Logs** — live, per-instance color, regex filter, follow toggle, export.
  - **Metrics** — cpu/mem/restart graphs.
  - **Config** — edit form, also shows the YAML.
  - **Events** — audit trail (started/restarted/crashed).
- **New worker** — **4-step wizard**: Command → Mode → Recovery → Review, with
  framework auto-detect hints ("Detected Laravel — suggest daemon ×4, auto-fill?").
- Cross-platform native look via Tauri; light/dark theme.

## 8. Security & IPC

- **Transport** — Unix domain socket (perms `0600`, user-only) on macOS/Linux; named pipe
  (ACL user-only) on Windows. No TCP port by default → not network-reachable.
- **Auth** — daemon generates a token at install (`0600` in app data dir); GUI sends it
  with each request. Blocks other local apps from driving the daemon.
- **Protocol** — JSON-RPC-style over the socket: request/response + a subscribe channel
  for log/state streaming. Versioned (newer GUI ↔ older daemon degrades gracefully).
- **Privilege model** — daemon runs as the same user as the workers (no root). Default
  install is per-user (LaunchAgent / systemd `--user`) → no admin needed. System-wide
  (runs before login) is an optional advanced install.
- **Command injection** — commands spawned as arg vectors (exec-style), not shell strings,
  by default. Optional, clearly-flagged "run in shell" per worker for pipes/globs.
- **Secrets** — env/`.env` may hold secrets; stored in SQLite, with optional OS keychain
  (Keychain / Credential Manager / libsecret) for marked-secret vars (later). Optional
  secret-masking in logs.
- **Product path** — remote control is out of v1; when added → TLS + real auth.

## 9. Build, Testing, Packaging, Updates

### Repo layout (Cargo workspace + Tauri)
```
servicio/
  crates/
    servicio-core/    # supervisor engine: process, run modes, restart, scheduler, health
    servicio-daemon/  # service binary: IPC server, SQLite, OS-service install
    servicio-ipc/     # shared protocol types (req/resp/events)
    servicio-detect/  # pluggable framework detectors
  apps/
    desktop/          # Tauri app (Rust shell) + React/TS frontend
  cli/                # servicio CLI (thin IPC client, later)
```
`servicio-core` is pure and testable — no UI, no OS-service deps. Platform bits (signals,
service install, IPC transport) sit behind traits and are mockable.

### Testing
- **Unit** — state machine, backoff math, cron parsing, detectors (pure, fast).
- **Integration** — daemon spawns cheap test processes (sleep, crash-on-cmd, log-spam),
  asserting respawn, crash-loop stop, schedule firing, graceful stop, log capture, and
  DB reconcile-after-restart.
- **Cross-platform CI** — GitHub Actions matrix on macOS / Windows / Linux (process
  control differs most; must test on each).
- **Frontend** — component tests (Vitest), key flows with IPC mocked.
- **TDD** for the core engine (the risky part).

### Packaging
- Tauri bundler → `.dmg`/`.app`, `.msi`/`.exe`, `.AppImage`/`.deb`/`.rpm`.
- Installer registers the daemon service (LaunchAgent / Windows Service / systemd user
  unit) and sets autostart.
- Code signing + notarization (mac), Authenticode (Win) for distribution; dev builds
  unsigned.

### Updates
- Tauri built-in updater (signed manifest) for the GUI.
- Daemon self-update: GUI ships the new daemon binary; daemon does a staged self-replace +
  graceful restart, with workers restored via DB reconcile. Versioned IPC keeps a
  newer-GUI / older-daemon window working during the update.

## 10. Build phases (internal sequence; ship = full feature set)

1. Core engine + daemon (daemon mode, restart, logs) — headless, CLI-tested.
2. IPC + minimal GUI (dashboard, start/stop, live logs).
3. Scheduled + batch modes, health checks.
4. Wizard, metrics graphs, notifications.
5. Config import/export + detectors.
6. Packaging, service install, signing, updater.
7. Polish, secrets/keychain, audit views.

## 11. Open questions / future
- Worker dependencies / ordering (start B after A healthy) — not in v1.
- Resource limits enforcement (CPU/mem caps) — stubbed in model, enforced later.
- Remote/multi-machine control + auth — product tier.
- Plugin API for third-party detectors — after core detectors prove the trait.
