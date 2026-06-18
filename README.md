<div align="center">

<img src="apps/desktop/src-tauri/icons/128x128.png" alt="Servicio" width="96" height="96" />

# Servicio

**A friendly desktop supervisor for your long-running developer services.**

Keep Laravel queues, Python workers, Node scripts — *any* command — alive, restarted on crash, scheduled on cron, and streaming their logs and metrics, all from a calm control-room GUI.

[![CI](https://github.com/AdnanHussainTurki/servicio/actions/workflows/ci.yml/badge.svg)](https://github.com/AdnanHussainTurki/servicio/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/AdnanHussainTurki/servicio?sort=semver)](https://github.com/AdnanHussainTurki/servicio/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB.svg)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org)

</div>

<div align="center">

![Servicio dashboard](docs/images/dashboard-dark.png)

</div>

> **Note** — replace `AdnanHussainTurki` in the badge/links above with your GitHub user/org after you push.

---

## What is Servicio?

Every developer ends up with a drawer full of `php artisan queue:work`, `python worker.py`, `npm run dev`, cron jobs and one-off scripts — each in its own terminal tab, each forgotten until it silently dies. **Servicio** is a small, native desktop app (plus a headless daemon) that supervises all of them for you:

- It **keeps them running** — restart-on-crash with exponential backoff.
- It **starts on login** — a launchd / systemd user service, no `sudo`.
- It **shows you everything** — live logs, CPU & memory graphs, per-group rollups.
- It **finds your workers for you** — point it at a project folder and it detects Laravel queues, Procfiles, crontabs, VS Code tasks, Node and Python entrypoints.

No YAML-by-hand, no `tmux` gymnastics, no "is it still up?" — just a dashboard.

## Screenshots

| Dashboard (live CPU/memory) | Groups (per-group stats + bulk actions) |
| --- | --- |
| ![Dashboard](docs/images/dashboard-light.png) | ![Groups](docs/images/groups.png) |

| New-worker wizard (autodetect) | Dark mode |
| --- | --- |
| ![Wizard](docs/images/wizard.png) | ![Dark dashboard](docs/images/dashboard-dark.png) |

## Features

- **🟢 Supervision** — auto-restart with exponential backoff, restart counters, graceful stop, crash surfacing (the failure reason lands in the worker log, not the void).
- **⚙️ Three run modes** — **Daemon** (N always-on instances), **Scheduled** (cron or interval, with overlap policy), and **Batch** (run N times).
- **🔍 Autodetect** — scan a project folder and get ready-to-run suggestions from six detectors: **Laravel** (reads the real `QUEUE_CONNECTION`), **Procfile**, **crontab**, **VS Code `tasks.json`**, **Node** (`package.json` scripts) and **Python**.
- **🗂️ Groups & tags** — organize workers into groups (folder view + drill-in) and label them with tags; filter the dashboard by tag.
- **📊 Live telemetry** — per-instance CPU & memory sampling, fleet-wide and per-group rollups, rolling sparklines.
- **⏯️ Bulk actions** — start / stop / restart a whole group in one click.
- **✏️ Full CRUD** — create with a wizard, **edit** any worker (editable display name, immutable id), **delete**, and **import / export** your whole config as JSON.
- **🔔 Diagnostics** — a daemon log written to disk, a Settings → Debug viewer, and optional **Sentry** error reporting.
- **🔄 Self-updating** — Tauri auto-updater wired in; the GUI even auto-replaces a stale daemon after an upgrade.
- **🖥️ Native & cross-platform** — a universal macOS `.dmg` today; Linux (systemd) supported; Windows on the roadmap.

## Architecture

Servicio is a Rust **cargo workspace** plus a **Tauri** desktop app:

```
┌─────────────────────────────────────────────────────────┐
│  apps/desktop        Tauri 2 + React 19 + TS (the GUI)   │
│     └─ src-tauri      Rust bridge → talks to the daemon  │
└───────────────┬─────────────────────────────────────────┘
                │ JSONL over a 0600 Unix socket (token auth)
┌───────────────▼─────────────────────────────────────────┐
│  servicio-daemon     headless supervisor + SQLite store  │
│     ├─ servicio-core    engine: spawn, restart, schedule │
│     ├─ servicio-ipc     framed JSONL protocol            │
│     ├─ servicio-detect  6 framework detectors            │
│     └─ servicio-cli     `servicio` terminal client       │
└─────────────────────────────────────────────────────────┘
```

| Crate | Responsibility |
| --- | --- |
| `servicio-core` | Process spawning, supervision, restart/backoff, run modes, metrics. |
| `servicio-ipc` | The JSONL `Frame` protocol shared by daemon and clients. |
| `servicio-daemon` | The long-lived supervisor: SQLite-backed worker store + socket server. |
| `servicio-cli` | `servicio` — a thin terminal client over the same protocol. |
| `servicio-detect` | Folder scanners that propose workers (Laravel, Node, Python, …). |

The **daemon** runs headless as a login-start user service; the **GUI** auto-spawns it as a sidecar and streams state/log/metric events over the socket.

## Install

### macOS

1. Download `servicio_<version>_universal.dmg` from the [latest release](https://github.com/AdnanHussainTurki/servicio/releases).
2. Open the `.dmg` and drag **Servicio** to Applications.
3. First launch: because the build is currently **unsigned**, right-click → **Open** (or run `xattr -dr com.apple.quarantine /Applications/servicio.app`).

> Signed & notarized builds require an Apple Developer certificate — see [`docs/RELEASING.md`](docs/RELEASING.md).

### Linux

Build from source (below); the daemon installs as a `systemd --user` unit. Packaged Linux artifacts are on the roadmap.

### Windows

On the roadmap — the engine is cross-platform; the Windows service wrapper is not yet implemented.

## Build from source

**Prerequisites:** [Rust](https://rustup.rs) (stable), [Node 20+](https://nodejs.org), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS (Xcode CLT on macOS; `webkit2gtk` + friends on Linux).

```bash
git clone https://github.com/AdnanHussainTurki/servicio.git
cd servicio

# 1. Engine + CLI + daemon (Rust workspace)
cargo build --release
cargo test                    # 106 tests

# 2. Desktop app (from apps/desktop)
cd apps/desktop
npm ci
npm run test                  # frontend tests
npm run tauri dev             # run the app against a dev daemon
```

To produce the distributable **universal macOS** bundle:

```bash
cd apps/desktop
PATH="$HOME/.cargo/bin:$PATH" npm run build:universal
# → src-tauri/target/universal-apple-darwin/release/bundle/dmg/servicio_<ver>_universal.dmg
```

## Usage

1. **Launch Servicio** — it starts (and supervises) the background daemon for you.
2. **Add a worker** — click **+ New worker**, point the wizard at a project folder, and pick from the detected suggestions, or enter a command by hand.
3. **Choose a run mode** — always-on daemon, cron/interval schedule, or a fixed batch.
4. **Watch it run** — open a worker for live logs and CPU/memory; group related workers and start/stop them together.
5. **Keep it running** — toggle **Start on login** in Settings so your services come back after a reboot.

Prefer the terminal? The same daemon speaks to the `servicio` CLI:

```bash
servicio ps                   # show workers + state
servicio start queue          # start / stop a worker
servicio logs queue           # stream a worker's logs
servicio metrics queue        # CPU / memory samples
servicio detect ~/app         # scan a folder for worker suggestions
servicio daemon-log           # the daemon's own diagnostics
```

### Error reporting (optional)

Set a Sentry DSN before the daemon starts to capture panics and worker failures:

```bash
SERVICIO_SENTRY_DSN="https://...@sentry.io/123" servicio-daemon serve
```

## Roadmap

- Windows service wrapper + packaged Linux artifacts
- Tag-based AND-filtering and saved dashboard views
- Symfony / Rails / docker-compose detectors
- Hosted update endpoint + signed/notarized releases

See the [design specs](docs/superpowers/specs/) for the full history and deferred items.

## Contributing

Contributions are very welcome — bug reports, features, detectors, docs, all of it. Start with **[CONTRIBUTING.md](CONTRIBUTING.md)**: it covers the dev setup, how to run each part, and the code-style gates (`cargo fmt`, `clippy -D warnings`, `eslint` clean — all enforced in CI).

The short version:

```bash
cargo test && (cd apps/desktop && npm run test && npm run lint)
```

Then fork → branch → open a PR against `main`. CI must be green.

## Contributors

Thanks goes to these wonderful people:

<!-- Add contributors here, e.g. via https://allcontributors.org -->
<a href="https://github.com/AdnanHussainTurki/servicio/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=AdnanHussainTurki/servicio" alt="Contributors" />
</a>

*Be the first — your PR puts you here.*

## Support / Donations

Servicio is free and MIT-licensed. If it saves you a few terminal tabs and you'd like to say thanks, sponsorship keeps the work going:

- 💖 **GitHub Sponsors** — *enable Sponsors and add your link*
- ☕ **Ko-fi / Buy Me a Coffee** — *add your handle*

Funding links live in [`.github/FUNDING.yml`](.github/FUNDING.yml) — uncomment and fill in the platforms you use, and a **Sponsor** button appears on the repo automatically. Stars and shares help just as much. ⭐

## License

[MIT](LICENSE) © 2026 Adnan Hussain.

<div align="center">
<sub>Built with Rust, Tauri, and React. <code>SERVICIO</code></sub>
</div>
