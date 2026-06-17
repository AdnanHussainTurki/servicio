# Servicio

Cross-platform desktop supervisor for local service workers (Laravel queues, Python
scripts, any command). See `docs/superpowers/specs/2026-06-18-servicio-design.md` for the
full design.

## Phase 1 (this milestone)

Headless Rust supervisor engine + daemon with SQLite persistence and a test CLI.

### Build & test

```bash
cargo build
cargo test
```

### Try it

The `--args` flag is a greedy trailing argument: put it LAST, and pass each worker
argument as a separate token.

```bash
# add an always-on worker (2 instances)
cargo run -p servicio-daemon -- --db servicio.db \
  add --name ticker --command sh --concurrency 2 \
  --args -c "while true; do echo tick; sleep 1; done"

# list workers
cargo run -p servicio-daemon -- --db servicio.db list

# supervise autostart workers until Ctrl-C (logs in $TMPDIR/servicio-logs)
cargo run -p servicio-daemon -- --db servicio.db run
```

### Crates
- `servicio-core` — supervisor engine (pure, fully unit-tested): run modes, restart
  policy with exponential backoff + crash-loop guard, concurrent stdout/stderr log
  capture with rotation, instance state machine, process abstraction, multi-worker manager.
- `servicio-daemon` — SQLite persistence + reconcile + test CLI.

Next phases: IPC + Tauri GUI, scheduled/batch run modes, framework autodetect, OS-service
install + run-on-boot, packaging.
