# Servicio

Cross-platform desktop supervisor for local service workers (Laravel queues, Python
scripts, any command). See `docs/superpowers/specs/2026-06-18-servicio-design.md` for the
full design.

## Status

- **Phase 1 (done):** headless Rust supervisor engine + daemon with SQLite persistence.
- **Phase 2a (done):** daemon `serve` command exposing a local authenticated socket API
  (JSONL over a Unix domain socket) + a thin `servicio` CLI client. Live state/log events.
- **Next:** Phase 2b Tauri GUI; then scheduled/batch modes, framework autodetect,
  OS-service install + run-on-boot, packaging.

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
