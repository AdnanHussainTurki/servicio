# Servicio Phase 2a — IPC Layer Design Spec

**Date:** 2026-06-18
**Status:** Approved (design); implementation plan pending
**Builds on:** Phase 1 (`servicio-core` engine + `servicio-daemon` persistence/CLI, merged to main).
**Parent design:** `docs/superpowers/specs/2026-06-18-servicio-design.md`

## 1. Summary

Phase 2a gives the daemon a local network API so external clients (a CLI now, the Tauri
GUI in Phase 2b) can drive it and watch workers live — without any GUI yet. It is built
and tested entirely headless, end-to-end over a real socket, so the protocol is proven
before the GUI rides on it.

It also lands the Phase-1 deferred item: wiring the instance state machine into the
supervisor, since lifecycle states finally get consumed here (streamed to clients).

### Goals
- Daemon exposes a local, authenticated socket API: list/add/remove/start/stop workers,
  subscribe to live state + log events, query daemon info.
- A `serve` command runs the daemon as a long-lived process with single-instance locking
  and graceful shutdown.
- A thin `servicio` CLI client proves the API end-to-end.
- Wire the state machine into the supervisor; emit state-transition events.

### Non-goals (2a)
- Tauri GUI (Phase 2b).
- Windows named-pipe transport (trait-gated; Unix socket implemented first).
- OS-service install / auto-start-on-boot (later phase).
- Remote/multi-machine control, TLS (product tier).
- Scheduled/batch run modes, metrics history, notifications.

## 2. Decisions (from brainstorming)

| Topic | Decision |
|---|---|
| Phase split | 2a = IPC headless; 2b = Tauri GUI on top. |
| Daemon launch | Manual `servicio-daemon serve` (no auto-spawn, no service install yet). |
| Transport | Unix domain socket, perms `0600`. Named pipe deferred (trait-gated). |
| Encoding | Newline-delimited JSON (JSONL), one JSON object per line. |
| Framing | Tagged enum: `Request{id,method,params}` / `Response{id,...}` / `Event{topic,payload}`. |
| Auth | First frame must be `hello` with a token; bad/missing → reject + close. |
| State machine | Wire into supervisor here; reconcile transition table with real emissions. |
| Event delivery | One connection; `tokio::sync::broadcast`; slow clients lag-drop, never block supervisor. |

## 3. Architecture

```
  servicio (CLI)                      servicio-daemon serve
  ┌────────────┐   JSONL over UDS     ┌─────────────────────────┐
  │ thin client│◄───────────────────►│ accept loop (1 task/conn)│
  └────────────┘   hello+token        │   ├─ handshake/auth      │
                                       │   ├─ method dispatch     │
                                       │   └─ event fan-out       │
                                       │ Manager (servicio-core)  │
                                       │   ├─ supervisors ────────┼─► state+log broadcast
                                       │   └─ SQLite (Db)         │
                                       └─────────────────────────┘
```

- **`servicio-ipc`** (new, pure): protocol types + framing helpers. No tokio, no IO.
- **`servicio-daemon`**: gains a `serve` subcommand — socket server, auth, single-instance
  lock, method dispatch over `Manager`+`Db`, event fan-out, graceful shutdown.
- **`servicio-cli`** (new): thin `servicio` binary; connects, handshakes, issues requests,
  renders responses/events.
- **`servicio-core`**: gains a status snapshot API on `Manager`, a `broadcast` event channel
  populated by supervisors, and the state-machine wiring.

### Crate layout (additions in bold)
```
crates/
  servicio-core/      # engine (+ status API, event broadcast, state-machine wiring)
  servicio-ipc/       # NEW: Request/Response/Event + framing
  servicio-daemon/    # + serve command (server, lock, auth, dispatch, shutdown)
  servicio-cli/       # NEW: `servicio` client binary
```

## 4. Protocol (`servicio-ipc`)

JSONL frames over the socket. One `Frame` per line.

```rust
enum Frame {
    Request  { id: u64, method: String, params: serde_json::Value },
    Response { id: u64, result: Option<Value>, error: Option<ApiError> },
    Event    { topic: String, payload: Value },
}

struct ApiError { code: String, message: String }
```

Typed params/results live as serde structs in `servicio-ipc` (e.g. `AddWorkerParams`,
`WorkerStatus`, `StateEvent`, `LogEvent`), serialized into/out of the `Value` slots, so the
daemon and CLI share one definition.

### Methods (v1)
| Method | Params | Result |
|---|---|---|
| `hello` | `{ token }` | `{ daemon_version }` (or error → connection closed) |
| `ping` | — | `{ "pong": true }` |
| `daemon_info` | — | `{ version, uptime_secs, worker_count, running_count }` |
| `list_workers` | — | `[ WorkerStatus ]` |
| `add_worker` | `WorkerSpec` | `{ name }` (persists to SQLite) |
| `remove_worker` | `{ name }` | `{ removed: bool }` (stops if running) |
| `start_worker` | `{ name }` | `{ started: bool }` |
| `stop_worker` | `{ name }` | `{ stopped: bool }` |
| `subscribe` | `{ topics: ["state","log"], worker?: name }` | `{ subscribed: true }` then `Event`s |

`WorkerStatus` = `{ name, run_mode, state, instances, running, restart_count, pids, uptime_secs }`.

### Events
- `Event{ topic:"state", payload: StateEvent{ worker, instance, from, to } }`
- `Event{ topic:"log",   payload: LogEvent{ worker, instance, stream, line } }`
- `Event{ topic:"lagged", payload: { dropped: u64 } }` when a slow client misses broadcast items.

### Auth handshake
On connect, the daemon expects the first frame to be `Request{method:"hello", params:{token}}`.
Wrong/missing token → `Response` with `ApiError{code:"unauthorized"}` then the daemon closes
the connection. The token is generated on first `serve` and stored `0600` in the app-data dir;
the CLI reads the same file. No token in the clear anywhere else.

## 5. Daemon `serve`

Replaces the Phase-1 blocking `run`. Startup sequence:
1. **Single-instance lock** — `flock` an exclusive lockfile beside the socket. If held by a
   live daemon, exit with a clear message. If stale (no live holder), reclaim: remove stale
   socket + lock and continue.
2. **Token** — generate if absent (CSPRNG, hex), store `0600`; else read existing.
3. **Socket** — bind Unix domain socket at the app-data path, `chmod 0600`.
4. **Reconcile** — open SQLite, start `autostart` workers via `Manager` (reuses Phase-1
   `reconcile_specs`).
5. **Accept loop** — each connection is a Tokio task: read frames line-by-line, require
   `hello` first, then dispatch methods; `subscribe` attaches a `broadcast::Receiver` and
   forwards events as `Event` frames interleaved with responses.
6. **Shutdown** — on SIGTERM/SIGINT: stop workers (Phase-1 `stop_all`, now awaiting child
   reaping), remove socket + lock, exit 0.

### Manager / supervisor additions (`servicio-core`)
- `Manager::status()` → `Vec<WorkerStatusCore>` (state, instance count, restart count, pids,
  uptime). Pure core type; mapped to the ipc `WorkerStatus` in the daemon.
- An event channel: `Manager` owns a `broadcast::Sender<SupervisorEvent>`; each
  `InstanceSupervisor` gets a clone and publishes `State{from,to}` and `Log{stream,line}`
  events. Bounded; lag surfaces as a `lagged` event to clients, never blocks the supervisor.
- **State-machine wiring:** `set_state` routes through `InstanceState::transition_to`. The
  transition table is reconciled with the supervisor's real emissions — notably adding the
  legal clean-exit edges `Running→Stopped` and `Starting→Stopped` — so no legitimate
  transition is rejected. Each accepted transition publishes a `State` event.

## 6. `servicio` CLI client (`servicio-cli`)

Thin binary that connects, handshakes with the token, and renders:
- `servicio ps` — `list_workers` as a table (name, mode, state, running/▢, restarts).
- `servicio add …` — `add_worker` (same fields as the Phase-1 daemon CLI add).
- `servicio start <name>` / `servicio stop <name>` — control.
- `servicio logs <name> [-f]` — `subscribe{topics:["log"],worker}` and stream lines; `-f`
  follows until Ctrl-C.
- `servicio info` — `daemon_info`.
- `--socket <path>` / `--token-file <path>` overrides for tests.

The Phase-1 `servicio-daemon` `add`/`list`/`run` subcommands: `run` is removed (replaced by
`serve`); `add`/`list` may stay as direct-DB conveniences or be dropped — the plan decides,
but the canonical path is now CLI→IPC→daemon.

## 7. Security

- Unix socket perms `0600`; not network-reachable (no TCP).
- Token required on every connection (`hello`); generated `0600`, never logged.
- Commands still spawned as arg vectors (Phase-1), not shell strings.
- Lockfile + socket live in the user's app-data/runtime dir, user-only.
- Windows named-pipe ACL + remote/TLS are future; the transport sits behind a trait so the
  Unix impl is swappable without touching dispatch logic.

## 8. Testing

- **`servicio-ipc` unit (pure):** Frame serde roundtrips for every variant; framing of
  partial lines and large payloads; `ApiError` paths.
- **`servicio-core` additions:** state-machine wiring asserts transition *sequences* via the
  broadcast (e.g. `Starting→Running→Crashed→Backoff→Starting…→Failed`), not just final state;
  slow-subscriber lag-drop emits `lagged` and never blocks the supervisor.
- **Daemon integration (real socket, `tempfile` paths):**
  - handshake: bad token rejected + closed; good token accepted.
  - `add_worker` → `list_workers` reflects it; `start_worker` produces `state` events;
    `stop_worker` → stopped.
  - `subscribe` + a crashing worker → receive `crashed`/`backoff`/`failed` events end-to-end.
  - single-instance lock: second `serve` fails with the expected error.
  - graceful shutdown removes socket + lock.
- **CLI integration:** spawn `serve` as a child process on a temp socket, drive
  `servicio ps/add/start/stop/logs`, assert rendered output; kill the child, confirm cleanup.
- TDD throughout. `servicio-ipc` has no tokio dependency.

## 9. Build phases (internal sequence)
1. `servicio-ipc` crate: Frame + typed params/results + framing helpers (pure, TDD).
2. `servicio-core`: `Manager::status()`, event broadcast, state-machine wiring (TDD).
3. `servicio-daemon`: socket server + handshake/auth + method dispatch (no events yet).
4. Single-instance lock + graceful shutdown.
5. Event fan-out: `subscribe` streams state + log events; lag handling.
6. `servicio-cli`: `ps/add/start/stop/info`, then `logs -f`.
7. CLI + daemon integration tests; docs/README update.

## 10. Open questions / future
- Keep or drop the Phase-1 direct-DB `add`/`list` daemon subcommands — plan decides.
- Reconnect/backoff in the CLI client when the daemon restarts — minimal for 2a (error out),
  richer in 2b GUI.
- Multiple concurrent subscribers fairness — broadcast handles it; revisit buffer sizing if
  log-heavy workers starve slow clients.
