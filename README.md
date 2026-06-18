# Servicio

Cross-platform desktop supervisor for local service workers (Laravel queues, Python
scripts, any command). See `docs/superpowers/specs/2026-06-18-servicio-design.md` for the
full design.

## Status

- **Phase 1 (done):** headless Rust supervisor engine + daemon with SQLite persistence.
- **Phase 2a (done):** daemon `serve` command exposing a local authenticated socket API
  (JSONL over a Unix domain socket) + a thin `servicio` CLI client. Live state/log events.
- **Phase 2b (done):** minimal Tauri desktop GUI — auto-spawns the daemon sidecar, card-grid
  dashboard with live status, worker detail + live logs, start/stop/restart, simple add form.
- **Next:** Phase 2c — creation wizard, framework autodetect UI, metrics graphs,
  notifications; then OS-service install + run-on-boot, packaging.

### Run the GUI (dev)

The GUI spawns the `servicio-daemon` binary as a sidecar, so build it first and put it on
PATH:

```bash
cargo build -p servicio-daemon          # provides target/debug/servicio-daemon
cd apps/desktop
npm install
PATH="$PWD/../../target/debug:$PATH" npm run tauri dev
```

The window opens, the daemon auto-starts under `$XDG_RUNTIME_DIR/servicio` (or a temp dir),
and the dashboard shows workers live. Add one with "+ New worker". GUI app lives in
`apps/desktop/` (React/TS frontend + `src-tauri/` Rust bridge); its tests run with
`npm run test` (frontend) and `cargo test` inside `apps/desktop/src-tauri` (bridge).

### Build & test

```bash
cargo build
cargo test
```

### Try it (Phase 2a)

Register a worker into the daemon's database, then run the daemon and drive it with the
`servicio` client. The `--args` flag is a greedy trailing argument: put it LAST, and pass
each worker argument as a separate token.

```bash
# register an always-on worker (2 instances) into the db under the daemon's base dir
cargo run -p servicio-daemon -- --db /tmp/servicio/servicio.db \
  add --name ticker --command sh --concurrency 2 \
  --args -c "while true; do echo tick; sleep 1; done"

# start the daemon (binds a 0600 socket under --base; Ctrl-C to stop)
cargo run -p servicio-daemon -- serve --base /tmp/servicio
```

In another terminal, drive it with the client:

```bash
cargo run -p servicio-cli -- --base /tmp/servicio ps
cargo run -p servicio-cli -- --base /tmp/servicio info
cargo run -p servicio-cli -- --base /tmp/servicio start ticker
cargo run -p servicio-cli -- --base /tmp/servicio logs ticker   # live, follow
cargo run -p servicio-cli -- --base /tmp/servicio stop ticker
```

### Crates
- `servicio-core` — supervisor engine (pure, fully unit-tested): run modes, restart
  policy with exponential backoff + crash-loop guard, concurrent stdout/stderr log
  capture with rotation, instance state machine + event broadcast, process abstraction,
  multi-worker manager with live status.
- `servicio-ipc` — pure wire protocol: JSONL `Frame` types + typed params/results/events.
- `servicio-daemon` — SQLite persistence + `serve` socket server (token auth,
  single-instance lock, method dispatch, event fan-out, graceful shutdown).
- `servicio-cli` — the `servicio` client binary (`ps`/`info`/`start`/`stop`/`logs`).
