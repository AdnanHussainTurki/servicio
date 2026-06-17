# Servicio Phase 2a — IPC Layer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the daemon a local, authenticated JSONL-over-Unix-socket API (list/add/remove/start/stop workers + subscribe to live state & log events), driven by a thin `servicio` CLI, with single-instance locking and graceful shutdown — all headless and TDD'd end-to-end. Also wire the instance state machine into the supervisor.

**Architecture:** A new pure `servicio-ipc` crate holds the protocol (`Frame` enum + typed params/results + line helpers). `servicio-core` gains an event broadcast (`tokio::sync::broadcast`) populated by supervisors, a `Manager::status()` snapshot, and state-machine wiring. `servicio-daemon` gains a `serve` command: a Unix-socket server that authenticates with a token, dispatches methods over `Manager`+`Db`, fans out events to subscribers, and shuts down gracefully. A new `servicio-cli` crate is a thin client.

**Tech Stack:** Rust, Tokio (UnixListener, broadcast, signal), serde/serde_json, clap, fs2 (file lock), getrandom (token), anyhow (CLI errors). Tests: `#[tokio::test]`, `tempfile`, real child processes.

**Builds on:** Phase 1 (merged). Spec: `docs/superpowers/specs/2026-06-18-servicio-phase2a-ipc-design.md`.

---

## File Structure

```
crates/
  servicio-core/
    src/
      event.rs        # NEW: SupervisorEvent, InstanceStatus, WorkerStatusCore
      supervisor.rs   # MOD: events sender, pid tracking, state-machine wiring, state()/pid()
      manager.rs      # MOD: broadcast channel, Arc<InstanceSupervisor> tracking, status(), subscribe()
      state.rs        # MOD: add legal edges Running->Stopped, Stopped->Failed
      lib.rs          # MOD: pub mod event;
  servicio-ipc/       # NEW crate
    src/
      lib.rs          # Frame, ApiError, framing helpers
      types.rs        # typed params/results: WorkerStatus, StateEvent, LogEvent, AddWorkerParams, ...
  servicio-daemon/
    src/
      token.rs        # NEW: generate/read auth token (0600)
      lock.rs         # NEW: single-instance lockfile (fs2)
      serve.rs        # NEW: socket server, handshake, dispatch, event fan-out, shutdown
      paths.rs        # NEW: socket/token/lock/db path resolution
      cli.rs          # MOD: add `serve` subcommand; remove `run`
      lib.rs          # MOD: pub mod token/lock/serve/paths
      main.rs         # MOD: wire `serve`
  servicio-cli/       # NEW crate: `servicio` binary
    src/
      main.rs         # clap: ps/add/start/stop/info/logs
      client.rs       # connect + handshake + request/response + event stream
```

Workspace `members` gains `crates/servicio-ipc` and `crates/servicio-cli`.

---

## Task 0: `servicio-ipc` crate — Frame + line helpers (TDD)

**Files:**
- Modify: `Cargo.toml` (workspace) — add members + shared deps
- Create: `crates/servicio-ipc/Cargo.toml`, `crates/servicio-ipc/src/lib.rs`

- [ ] **Step 1: Add workspace deps + member**

Edit root `Cargo.toml`: set
```toml
members = ["crates/servicio-core", "crates/servicio-daemon", "crates/servicio-ipc"]
```
(`servicio-cli` is added in Task 7.) And add to `[workspace.dependencies]`:
```toml
fs2 = "0.4"
getrandom = "0.2"
anyhow = "1"
```

- [ ] **Step 2: Create the ipc crate manifest**

Create `crates/servicio-ipc/Cargo.toml`:
```toml
[package]
name = "servicio-ipc"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
```

- [ ] **Step 3: Write the failing tests**

Create `crates/servicio-ipc/src/lib.rs`:
```rust
//! servicio-ipc: the wire protocol shared by the daemon and clients.
//! Pure types + line framing. No tokio, no IO.

pub mod types;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One protocol message. Encoded as a single JSON object on its own line (JSONL).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Frame {
    Request { id: u64, method: String, params: Value },
    Response { id: u64, result: Option<Value>, error: Option<ApiError> },
    Event { topic: String, payload: Value },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl Frame {
    /// Serialize to a single line WITHOUT the trailing newline.
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("frame serializes")
    }

    /// Parse one line (newline already stripped) into a Frame.
    pub fn from_line(line: &str) -> Result<Frame, serde_json::Error> {
        serde_json::from_str(line)
    }

    pub fn ok(id: u64, result: Value) -> Frame {
        Frame::Response { id, result: Some(result), error: None }
    }

    pub fn err(id: u64, code: &str, message: &str) -> Frame {
        Frame::Response {
            id,
            result: None,
            error: Some(ApiError { code: code.into(), message: message.into() }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_roundtrips_and_has_no_newline() {
        let f = Frame::Request { id: 7, method: "ping".into(), params: json!({}) };
        let line = f.to_line();
        assert!(!line.contains('\n'));
        assert_eq!(Frame::from_line(&line).unwrap(), f);
    }

    #[test]
    fn ok_and_err_helpers_build_responses() {
        assert_eq!(
            Frame::ok(1, json!({"pong": true})),
            Frame::Response { id: 1, result: Some(json!({"pong": true})), error: None }
        );
        match Frame::err(2, "unauthorized", "bad token") {
            Frame::Response { id: 2, result: None, error: Some(e) } => {
                assert_eq!(e.code, "unauthorized");
            }
            _ => panic!("expected error response"),
        }
    }

    #[test]
    fn event_frame_roundtrips() {
        let f = Frame::Event { topic: "state".into(), payload: json!({"worker": "q"}) };
        assert_eq!(Frame::from_line(&f.to_line()).unwrap(), f);
    }

    #[test]
    fn malformed_line_is_an_error_not_a_panic() {
        assert!(Frame::from_line("{not json").is_err());
    }
}
```

- [ ] **Step 4: Create a stub `types` module so it compiles**

Create `crates/servicio-ipc/src/types.rs`:
```rust
// Filled in Task 1.
```

- [ ] **Step 5: Run, confirm FAIL then PASS**

Run: `cargo test -p servicio-ipc`
Expected: compiles; 4 tests PASS. (If `types` module errors, ensure the stub file exists.)

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/servicio-ipc
git commit -m "feat(ipc): protocol Frame enum + JSONL line helpers"
```

---

## Task 1: `servicio-ipc` typed params/results (TDD)

**Files:**
- Modify: `crates/servicio-ipc/src/types.rs`
- Modify: `crates/servicio-ipc/Cargo.toml` (add servicio-core dep for shared enums)

We reuse `RunMode` and `InstanceState` from `servicio-core` so the wire types match the engine exactly.

- [ ] **Step 1: Depend on servicio-core**

Edit `crates/servicio-ipc/Cargo.toml`, add under `[dependencies]`:
```toml
servicio-core = { path = "../servicio-core" }
```

- [ ] **Step 2: Write the failing tests**

Replace `crates/servicio-ipc/src/types.rs`:
```rust
use serde::{Deserialize, Serialize};
use servicio_core::state::InstanceState;
use servicio_core::worker::{RunMode, WorkerSpec};

/// Status of one worker as reported by `list_workers`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerStatus {
    pub name: String,
    pub run_mode: RunMode,
    pub instances: Vec<InstanceStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceStatus {
    pub index: u32,
    pub state: InstanceState,
    pub restart_count: u32,
    pub pid: Option<u32>,
}

/// Params for `add_worker` — a full worker definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddWorkerParams {
    pub spec: WorkerSpec,
}

/// Params for the single-name methods (start/stop/remove).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NameParams {
    pub name: String,
}

/// Params for `hello`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HelloParams {
    pub token: String,
}

/// Params for `subscribe`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubscribeParams {
    pub topics: Vec<String>,
    #[serde(default)]
    pub worker: Option<String>,
}

/// Event payloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateEvent {
    pub worker: String,
    pub instance: u32,
    pub from: InstanceState,
    pub to: InstanceState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEvent {
    pub worker: String,
    pub instance: u32,
    pub stream: String,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonInfo {
    pub version: String,
    pub uptime_secs: u64,
    pub worker_count: u32,
    pub running_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn worker_status_roundtrips() {
        let s = WorkerStatus {
            name: "q".into(),
            run_mode: RunMode::Daemon { concurrency: 2 },
            instances: vec![InstanceStatus {
                index: 0,
                state: InstanceState::Running,
                restart_count: 1,
                pid: Some(4321),
            }],
        };
        let v = serde_json::to_value(&s).unwrap();
        let back: WorkerStatus = serde_json::from_value(v).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn subscribe_params_default_worker_is_none() {
        let p: SubscribeParams = serde_json::from_value(json!({"topics": ["state"]})).unwrap();
        assert_eq!(p.worker, None);
    }

    #[test]
    fn state_event_roundtrips() {
        let e = StateEvent {
            worker: "q".into(),
            instance: 0,
            from: InstanceState::Starting,
            to: InstanceState::Running,
        };
        let back: StateEvent = serde_json::from_value(serde_json::to_value(&e).unwrap()).unwrap();
        assert_eq!(e, back);
    }
}
```

- [ ] **Step 3: Make core types public as needed**

Confirm `servicio-core/src/lib.rs` exposes `pub mod state;` and `pub mod worker;` (they do, from Phase 1). `InstanceState`, `RunMode`, `WorkerSpec` are already `pub`.

- [ ] **Step 4: Run, confirm PASS**

Run: `cargo test -p servicio-ipc`
Expected: all tests PASS (4 from Task 0 + 3 here).

- [ ] **Step 5: Commit**

```bash
git add crates/servicio-ipc
git commit -m "feat(ipc): typed params + result + event structs"
```

---

## Task 2: `servicio-core` event broadcast + status + pid (TDD)

**Files:**
- Create: `crates/servicio-core/src/event.rs`
- Modify: `crates/servicio-core/src/lib.rs`, `supervisor.rs`, `manager.rs`

- [ ] **Step 1: Add the event module**

Create `crates/servicio-core/src/event.rs`:
```rust
use crate::state::InstanceState;
use crate::worker::RunMode;
use serde::{Deserialize, Serialize};

/// An event emitted by a supervisor onto the manager's broadcast channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SupervisorEvent {
    State { worker: String, instance: u32, from: InstanceState, to: InstanceState },
    Log { worker: String, instance: u32, stream: String, line: String },
}

/// Snapshot of one instance for status reporting.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceStatus {
    pub index: u32,
    pub state: InstanceState,
    pub restart_count: u32,
    pub pid: Option<u32>,
}

/// Snapshot of one worker (all its instances).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkerStatusCore {
    pub name: String,
    pub run_mode: RunMode,
    pub instances: Vec<InstanceStatus>,
}
```

- [ ] **Step 2: Export it**

In `crates/servicio-core/src/lib.rs`, add `pub mod event;` to the module list (alphabetical: after `pub mod error;`).

- [ ] **Step 3: Write the failing test (supervisor emits State events + tracks pid)**

Add this test to the `#[cfg(test)] mod tests` block in `crates/servicio-core/src/supervisor.rs`:
```rust
    #[tokio::test]
    async fn emits_state_events_and_tracks_terminal_state() {
        use crate::event::SupervisorEvent;
        let dir = tempfile::tempdir().unwrap();
        let policy = RestartPolicy { kind: RestartKind::Never, ..Default::default() };
        let (tx, mut rx) = tokio::sync::broadcast::channel(64);
        let sup = InstanceSupervisor::new(
            0,
            spec("sh", &["-c", "exit 0"], policy),
            Arc::new(TokioProcess),
            dir.path().join("t.log"),
        )
        .with_events(tx);
        sup.run_until_terminal().await;

        // Collect emitted state transitions.
        let mut seen = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            if let SupervisorEvent::State { from, to, .. } = ev {
                seen.push((from, to));
            }
        }
        // Must include Starting->Running and a terminal Running->Stopped.
        assert!(seen.contains(&(InstanceState::Starting, InstanceState::Running)));
        assert!(seen.contains(&(InstanceState::Running, InstanceState::Stopped)));
    }
```

- [ ] **Step 4: Run, confirm FAIL**

Run: `cargo test -p servicio-core emits_state_events`
Expected: FAIL — `no method named with_events`.

- [ ] **Step 5: Implement events + pid + state getter in the supervisor**

In `crates/servicio-core/src/supervisor.rs`:

(a) Extend the imports at the top:
```rust
use crate::event::SupervisorEvent;
use tokio::sync::broadcast;
```

(b) Add fields to the struct (after `state_rx`):
```rust
    events: Option<broadcast::Sender<SupervisorEvent>>,
    pid: AtomicU32,
```

(c) In `new`, initialize them — set `events: None` and `pid: AtomicU32::new(0)` in the constructed `Self { .. }`.

(d) Add a builder + getters in the `impl`:
```rust
    /// Attach an event sender so this supervisor broadcasts state/log events.
    pub fn with_events(mut self, tx: broadcast::Sender<SupervisorEvent>) -> Self {
        self.events = Some(tx);
        self
    }

    /// Current published state.
    pub fn state(&self) -> InstanceState {
        *self.state_rx.borrow()
    }

    /// Current OS pid (None if not running).
    pub fn pid(&self) -> Option<u32> {
        match self.pid.load(Ordering::SeqCst) {
            0 => None,
            p => Some(p),
        }
    }

    /// Worker name this instance belongs to.
    pub fn worker_name(&self) -> &str {
        &self.spec.name
    }
```

(e) Replace `set_state` so it validates the transition and emits a `State` event:
```rust
    fn set_state(&self, s: InstanceState) {
        let from = *self.state_rx.borrow();
        // Validate against the lifecycle machine. An illegal transition is a bug;
        // log it but still publish so observers stay consistent with reality.
        if from != s && !from.can_transition_to(s) {
            tracing::warn!("illegal instance transition {from:?} -> {s:?} for {}", self.spec.name);
        }
        let _ = self.state_tx.send(s);
        if let Some(tx) = &self.events {
            if from != s {
                let _ = tx.send(SupervisorEvent::State {
                    worker: self.spec.name.clone(),
                    instance: self.index,
                    from,
                    to: s,
                });
            }
        }
    }
```

(f) In `run_until_terminal`, record the pid right after a successful spawn (just after `self.set_state(InstanceState::Running);`):
```rust
            self.pid.store(spawned.pid().unwrap_or(0), Ordering::SeqCst);
```
and clear it right after `let status = spawned.wait().await;`:
```rust
            self.pid.store(0, Ordering::SeqCst);
```

(g) Emit log events from the pump. In the spawned pump task, the stdout/stderr writers currently only call `sink.write_line`. Also publish a `Log` event. Since the pump task needs the events sender + name + index, clone them in before spawning:
```rust
            let ev = self.events.clone();
            let name = self.spec.name.clone();
```
Then inside each of `out_fut` and `err_fut`, after `let _ = sink_out.lock().unwrap().write_line(idx, "stdout", &line);` add:
```rust
                            if let Some(tx) = &ev {
                                let _ = tx.send(SupervisorEvent::Log {
                                    worker: name.clone(),
                                    instance: idx,
                                    stream: "stdout".into(),
                                    line: line.clone(),
                                });
                            }
```
(and the analogous block with `"stderr"` in `err_fut`). Clone `ev`/`name` into each async block as needed (e.g. `let ev = ev.clone(); let name = name.clone();` before each future) so both closures own copies.

- [ ] **Step 6: Add legal state edges**

In `crates/servicio-core/src/state.rs`, in `can_transition_to`, add these arms to the `matches!` list:
```rust
                | (Running, Stopped)
                | (Stopped, Failed)
                | (Starting, Stopped)
```
(`Running→Stopped` = clean exit with no restart; `Stopped→Failed` = log-open failure from the initial state; `Starting→Stopped` = stopped mid-start.) Then add a test in `state.rs`'s test module:
```rust
    #[test]
    fn clean_exit_and_log_failure_edges_are_legal() {
        assert!(InstanceState::Running.can_transition_to(InstanceState::Stopped));
        assert!(InstanceState::Stopped.can_transition_to(InstanceState::Failed));
    }
```

- [ ] **Step 7: Run, confirm PASS**

Run: `cargo test -p servicio-core`
Expected: all Phase-1 tests + the new ones PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/servicio-core
git commit -m "feat(core): supervisor event broadcast, pid tracking, state-machine wiring"
```

---

## Task 3: `servicio-core` Manager broadcast + status (TDD)

**Files:**
- Modify: `crates/servicio-core/src/manager.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `crates/servicio-core/src/manager.rs`:
```rust
    #[tokio::test]
    async fn status_reports_running_instances() {
        let dir = tempfile::tempdir().unwrap();
        let mut mgr = Manager::new(Arc::new(TokioProcess), dir.path().to_path_buf());
        mgr.start_worker(long_running("q")).await;
        // give the instances a moment to reach Running
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let status = mgr.status();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].name, "q");
        assert_eq!(status[0].instances.len(), 2);
        mgr.stop_all().await;
    }

    #[tokio::test]
    async fn subscribe_receives_state_events() {
        use crate::event::SupervisorEvent;
        let dir = tempfile::tempdir().unwrap();
        let mut mgr = Manager::new(Arc::new(TokioProcess), dir.path().to_path_buf());
        let mut rx = mgr.subscribe();
        mgr.start_worker(long_running("q")).await;
        // Expect at least one State event within a short window.
        let got = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if let Ok(SupervisorEvent::State { .. }) = rx.recv().await {
                    break true;
                }
            }
        })
        .await
        .unwrap_or(false);
        assert!(got);
        mgr.stop_all().await;
    }
```

- [ ] **Step 2: Run, confirm FAIL**

Run: `cargo test -p servicio-core -p servicio-core status_reports`
Expected: FAIL — `no method named status` / `subscribe`.

- [ ] **Step 3: Implement**

Replace the body of `crates/servicio-core/src/manager.rs` ABOVE the test module with:
```rust
use crate::event::{InstanceStatus, SupervisorEvent, WorkerStatusCore};
use crate::process::ProcessSpawner;
use crate::supervisor::InstanceSupervisor;
use crate::worker::{RunMode, WorkerSpec};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

struct RunningWorker {
    spec: WorkerSpec,
    supervisors: Vec<Arc<InstanceSupervisor>>,
    handles: Vec<JoinHandle<()>>,
}

/// Owns all workers and their running instances. One Manager per daemon.
pub struct Manager {
    spawner: Arc<dyn ProcessSpawner>,
    log_dir: PathBuf,
    workers: HashMap<String, RunningWorker>,
    events: broadcast::Sender<SupervisorEvent>,
}

impl Manager {
    pub fn new(spawner: Arc<dyn ProcessSpawner>, log_dir: PathBuf) -> Self {
        let (events, _) = broadcast::channel(1024);
        Self { spawner, log_dir, workers: HashMap::new(), events }
    }

    /// Subscribe to the live event stream (state + log).
    pub fn subscribe(&self) -> broadcast::Receiver<SupervisorEvent> {
        self.events.subscribe()
    }

    /// Start every instance for a worker per its concurrency, each in its own task.
    pub async fn start_worker(&mut self, spec: WorkerSpec) {
        let concurrency = match spec.run_mode {
            RunMode::Daemon { concurrency } => concurrency.max(1),
        };
        let mut supervisors = Vec::new();
        let mut handles = Vec::new();
        for i in 0..concurrency {
            let log_path = self.log_dir.join(format!("{}-{}.log", spec.name, i));
            let sup = Arc::new(
                InstanceSupervisor::new(i, spec.clone(), Arc::clone(&self.spawner), log_path)
                    .with_events(self.events.clone()),
            );
            let run = Arc::clone(&sup);
            handles.push(tokio::spawn(async move { run.run_until_terminal().await }));
            supervisors.push(sup);
        }
        self.workers.insert(spec.name.clone(), RunningWorker { spec, supervisors, handles });
    }

    pub fn instance_count(&self, worker: &str) -> usize {
        self.workers.get(worker).map(|w| w.supervisors.len()).unwrap_or(0)
    }

    /// Snapshot of every worker's instances.
    pub fn status(&self) -> Vec<WorkerStatusCore> {
        let mut out: Vec<_> = self
            .workers
            .values()
            .map(|w| WorkerStatusCore {
                name: w.spec.name.clone(),
                run_mode: w.spec.run_mode.clone(),
                instances: w
                    .supervisors
                    .iter()
                    .map(|s| InstanceStatus {
                        index: s_index(s),
                        state: s.state(),
                        restart_count: s.restart_count(),
                        pid: s.pid(),
                    })
                    .collect(),
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// Stop one worker's instances. Returns true if it existed.
    pub async fn stop_worker(&mut self, name: &str) -> bool {
        if let Some(w) = self.workers.remove(name) {
            for h in w.handles {
                h.abort();
            }
            true
        } else {
            false
        }
    }

    /// Abort all running instance tasks (kill_on_drop terminates the children).
    pub async fn stop_all(&mut self) {
        for (_, w) in self.workers.drain() {
            for h in w.handles {
                h.abort();
            }
        }
    }
}

/// Read an instance's index for status. InstanceSupervisor exposes it via a getter.
fn s_index(s: &InstanceSupervisor) -> u32 {
    s.index()
}
```

- [ ] **Step 4: Add an `index()` getter to the supervisor**

In `crates/servicio-core/src/supervisor.rs`, add to the `impl InstanceSupervisor`:
```rust
    /// This instance's index within its worker.
    pub fn index(&self) -> u32 {
        self.index
    }
```

- [ ] **Step 5: Run, confirm PASS**

Run: `cargo test -p servicio-core`
Expected: all pass, including `status_reports_running_instances` and `subscribe_receives_state_events`. The Phase-1 `stop_all_removes_instances` test still passes (instance_count returns 0 after stop_all).

- [ ] **Step 6: Commit**

```bash
git add crates/servicio-core
git commit -m "feat(core): manager broadcast channel, status snapshot, per-worker stop"
```

---

## Task 4: daemon paths + token + lock (TDD)

**Files:**
- Create: `crates/servicio-daemon/src/paths.rs`, `token.rs`, `lock.rs`
- Modify: `crates/servicio-daemon/Cargo.toml`, `lib.rs`

- [ ] **Step 1: Add deps**

Edit `crates/servicio-daemon/Cargo.toml` `[dependencies]`, add:
```toml
servicio-ipc = { path = "../servicio-ipc" }
fs2.workspace = true
getrandom.workspace = true
```

- [ ] **Step 2: Path resolution module**

Create `crates/servicio-daemon/src/paths.rs`:
```rust
use std::path::PathBuf;

/// Resolved filesystem locations for one daemon instance, all under a base dir.
#[derive(Debug, Clone)]
pub struct Paths {
    pub base: PathBuf,
}

impl Paths {
    pub fn new(base: PathBuf) -> Self {
        Self { base }
    }

    /// Default base: $XDG_RUNTIME_DIR/servicio, else a temp-dir fallback.
    pub fn default_base() -> PathBuf {
        if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
            PathBuf::from(dir).join("servicio")
        } else {
            std::env::temp_dir().join("servicio")
        }
    }

    pub fn socket(&self) -> PathBuf { self.base.join("daemon.sock") }
    pub fn token(&self) -> PathBuf { self.base.join("token") }
    pub fn lock(&self) -> PathBuf { self.base.join("daemon.lock") }
    pub fn db(&self) -> PathBuf { self.base.join("servicio.db") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_under_base() {
        let p = Paths::new(PathBuf::from("/tmp/x"));
        assert_eq!(p.socket(), PathBuf::from("/tmp/x/daemon.sock"));
        assert_eq!(p.token(), PathBuf::from("/tmp/x/token"));
        assert_eq!(p.lock(), PathBuf::from("/tmp/x/daemon.lock"));
    }
}
```

- [ ] **Step 3: Token module — write the failing test**

Create `crates/servicio-daemon/src/token.rs`:
```rust
use std::fs;
use std::io;
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_then_reuses_stable_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        let a = load_or_create(&path).unwrap();
        let b = load_or_create(&path).unwrap();
        assert_eq!(a, b, "second call must reuse the stored token");
        assert_eq!(a.len(), 64, "32 random bytes hex-encoded = 64 chars");
    }

    #[cfg(unix)]
    #[test]
    fn token_file_is_user_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        load_or_create(&path).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
```

- [ ] **Step 4: Implement the token module**

Add ABOVE the test block in `crates/servicio-daemon/src/token.rs`:
```rust
/// Load the token from `path`, or generate + store a new one (0600) if absent.
pub fn load_or_create(path: &Path) -> io::Result<String> {
    if let Ok(existing) = fs::read_to_string(path) {
        let trimmed = existing.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    fs::write(path, &token)?;
    set_user_only(path)?;
    Ok(token)
}

#[cfg(unix)]
fn set_user_only(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_user_only(_path: &Path) -> io::Result<()> {
    Ok(())
}
```

- [ ] **Step 5: Lock module — write the failing test**

Create `crates/servicio-daemon/src/lock.rs`:
```rust
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_lock_on_same_path_fails_while_first_held() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.lock");
        let first = InstanceLock::acquire(&path).expect("first lock");
        let second = InstanceLock::acquire(&path);
        assert!(second.is_err(), "second acquire must fail while first is held");
        drop(first);
        // After releasing, a new acquire succeeds.
        let third = InstanceLock::acquire(&path);
        assert!(third.is_ok());
    }
}
```

- [ ] **Step 6: Implement the lock module**

Add ABOVE the test block in `crates/servicio-daemon/src/lock.rs`:
```rust
/// Holds an exclusive advisory lock on a lockfile for the daemon's lifetime.
/// Dropping it releases the lock.
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    pub fn acquire(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).read(true).write(true).open(path)?;
        file.try_lock_exclusive()
            .map_err(|_| io::Error::new(io::ErrorKind::AddrInUse, "another servicio daemon is already running"))?;
        Ok(Self { _file: file })
    }
}
```

- [ ] **Step 7: Export modules**

In `crates/servicio-daemon/src/lib.rs`, add:
```rust
pub mod lock;
pub mod paths;
pub mod token;
```
(keep the existing `pub mod cli; pub mod db;` etc.)

- [ ] **Step 8: Run, confirm PASS**

Run: `cargo test -p servicio-daemon paths token lock`
Expected: PASS — 4 tests (paths 1, token 2, lock 1).

- [ ] **Step 9: Commit**

```bash
git add crates/servicio-daemon Cargo.lock
git commit -m "feat(daemon): paths, auth token, single-instance lock"
```

---

## Task 5: daemon `serve` — server, handshake/auth, ping/info, shutdown (TDD)

**Files:**
- Create: `crates/servicio-daemon/src/serve.rs`
- Modify: `crates/servicio-daemon/src/{lib.rs,cli.rs,main.rs}`
- Create: `crates/servicio-daemon/tests/serve_integration.rs`

- [ ] **Step 1: Write the failing integration test**

Create `crates/servicio-daemon/tests/serve_integration.rs`:
```rust
// Spin up the server on a temp socket in-process, connect a raw client, and
// exercise the handshake + ping + second-instance lock.
use servicio_daemon_lib::paths::Paths;
use servicio_daemon_lib::serve::{serve, ServeHandle};
use servicio_ipc::Frame;
use serde_json::json;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

async fn start(paths: Paths, token: String) -> ServeHandle {
    let h = serve(paths, token).await.expect("serve starts");
    // Give the listener a beat to bind.
    tokio::time::sleep(Duration::from_millis(100)).await;
    h
}

async fn send_recv(sock: &std::path::Path, frames: &[Frame]) -> Vec<Frame> {
    let stream = UnixStream::connect(sock).await.unwrap();
    let (rd, mut wr) = stream.into_split();
    for f in frames {
        wr.write_all(format!("{}\n", f.to_line()).as_bytes()).await.unwrap();
    }
    let mut lines = BufReader::new(rd).lines();
    let mut out = Vec::new();
    for _ in 0..frames.len() {
        if let Ok(Some(line)) = lines.next_line().await {
            out.push(Frame::from_line(&line).unwrap());
        }
    }
    out
}

#[tokio::test]
async fn good_token_then_ping_works() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;

    let replies = send_recv(
        &paths.socket(),
        &[
            Frame::Request { id: 1, method: "hello".into(), params: json!({"token": "secret"}) },
            Frame::Request { id: 2, method: "ping".into(), params: json!({}) },
        ],
    )
    .await;

    assert!(matches!(replies[0], Frame::Response { id: 1, error: None, .. }));
    match &replies[1] {
        Frame::Response { id: 2, result: Some(v), .. } => assert_eq!(v["pong"], true),
        other => panic!("unexpected: {other:?}"),
    }
    h.shutdown().await;
}

#[tokio::test]
async fn bad_token_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;

    let replies = send_recv(
        &paths.socket(),
        &[Frame::Request { id: 1, method: "hello".into(), params: json!({"token": "wrong"}) }],
    )
    .await;
    match &replies[0] {
        Frame::Response { id: 1, error: Some(e), .. } => assert_eq!(e.code, "unauthorized"),
        other => panic!("expected unauthorized, got {other:?}"),
    }
    h.shutdown().await;
}

#[tokio::test]
async fn shutdown_removes_socket() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;
    assert!(paths.socket().exists());
    h.shutdown().await;
    assert!(!paths.socket().exists(), "socket file removed on shutdown");
}
```

- [ ] **Step 2: Run, confirm FAIL**

Run: `cargo test -p servicio-daemon --test serve_integration`
Expected: FAIL — unresolved `servicio_daemon_lib::serve`.

- [ ] **Step 3: Implement the server core**

Create `crates/servicio-daemon/src/serve.rs`:
```rust
use crate::db::Db;
use crate::paths::Paths;
use servicio_core::manager::Manager;
use servicio_core::process::TokioProcess;
use servicio_ipc::Frame;
use serde_json::json;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tokio::sync::watch;
use tokio::task::JoinHandle;

/// Shared daemon state handed to each connection.
pub struct Daemon {
    pub token: String,
    pub manager: Mutex<Manager>,
    pub db: Mutex<Db>,
    pub started: std::time::Instant,
    pub version: String,
}

/// Handle to a running server; used by tests and signal handling to stop it.
pub struct ServeHandle {
    shutdown_tx: watch::Sender<bool>,
    accept_task: JoinHandle<()>,
    socket_path: std::path::PathBuf,
}

impl ServeHandle {
    /// Signal the accept loop to stop, wait for it, and remove the socket.
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        let _ = self.accept_task.await;
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Bind the socket and start accepting connections in the background.
/// Reconciles autostart workers from the DB before returning.
pub async fn serve(paths: Paths, token: String) -> std::io::Result<ServeHandle> {
    std::fs::create_dir_all(&paths.base)?;
    // Fresh socket: remove any stale file first.
    let _ = std::fs::remove_file(paths.socket());
    let listener = UnixListener::bind(paths.socket())?;
    set_socket_perms(&paths.socket())?;

    let db = Db::open(&paths.db()).map_err(to_io)?;
    let log_dir = paths.base.join("logs");
    let mut manager = Manager::new(Arc::new(TokioProcess), log_dir);
    for spec in db.autostart_workers().map_err(to_io)? {
        manager.start_worker(spec).await;
    }

    let daemon = Arc::new(Daemon {
        token,
        manager: Mutex::new(manager),
        db: Mutex::new(db),
        started: std::time::Instant::now(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    });

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let socket_path = paths.socket();
    let accept_task = tokio::spawn(accept_loop(listener, daemon, shutdown_rx));

    Ok(ServeHandle { shutdown_tx, accept_task, socket_path })
}

async fn accept_loop(
    listener: UnixListener,
    daemon: Arc<Daemon>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() { break; }
            }
            accepted = listener.accept() => {
                if let Ok((stream, _addr)) = accepted {
                    let d = Arc::clone(&daemon);
                    tokio::spawn(handle_conn(stream, d));
                }
            }
        }
    }
}

async fn handle_conn(stream: UnixStream, daemon: Arc<Daemon>) {
    let (rd, mut wr) = stream.into_split();
    let mut lines = BufReader::new(rd).lines();
    let mut authed = false;

    while let Ok(Some(line)) = lines.next_line().await {
        let frame = match Frame::from_line(&line) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let Frame::Request { id, method, params } = frame else { continue };

        // First frame must be a successful `hello`.
        if !authed {
            if method == "hello" {
                let token_ok = params.get("token").and_then(|t| t.as_str()) == Some(daemon.token.as_str());
                if token_ok {
                    authed = true;
                    let reply = Frame::ok(id, json!({"daemon_version": daemon.version}));
                    let _ = write_frame(&mut wr, &reply).await;
                    continue;
                }
            }
            let reply = Frame::err(id, "unauthorized", "valid hello required");
            let _ = write_frame(&mut wr, &reply).await;
            return; // close connection
        }

        let reply = dispatch(&daemon, id, &method, params).await;
        if write_frame(&mut wr, &reply).await.is_err() {
            return;
        }
    }
}

/// Method dispatch for authenticated connections. Extended in later tasks.
async fn dispatch(daemon: &Arc<Daemon>, id: u64, method: &str, _params: serde_json::Value) -> Frame {
    match method {
        "ping" => Frame::ok(id, json!({"pong": true})),
        "daemon_info" => {
            let mgr = daemon.manager.lock().await;
            let status = mgr.status();
            let worker_count = status.len() as u32;
            let running_count = status
                .iter()
                .filter(|w| w.instances.iter().any(|i| matches!(i.state, servicio_core::state::InstanceState::Running)))
                .count() as u32;
            Frame::ok(
                id,
                json!({
                    "version": daemon.version,
                    "uptime_secs": daemon.started.elapsed().as_secs(),
                    "worker_count": worker_count,
                    "running_count": running_count,
                }),
            )
        }
        other => Frame::err(id, "unknown_method", &format!("no such method: {other}")),
    }
}

async fn write_frame(wr: &mut tokio::net::unix::OwnedWriteHalf, frame: &Frame) -> std::io::Result<()> {
    wr.write_all(format!("{}\n", frame.to_line()).as_bytes()).await
}

#[cfg(unix)]
fn set_socket_perms(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_socket_perms(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

fn to_io<E: std::fmt::Display>(e: E) -> std::io::Error {
    std::io::Error::other(e.to_string())
}
```

- [ ] **Step 4: Export the module**

In `crates/servicio-daemon/src/lib.rs`, add `pub mod serve;`.

- [ ] **Step 5: Run, confirm PASS**

Run: `cargo test -p servicio-daemon --test serve_integration`
Expected: PASS — 3 tests (good token + ping, bad token rejected, shutdown removes socket).

- [ ] **Step 6: Wire the `serve` subcommand**

In `crates/servicio-daemon/src/cli.rs`: remove the `Run` variant and add:
```rust
    /// Run the daemon: bind the socket and supervise workers until terminated.
    Serve {
        /// Base dir for socket/token/lock/db (defaults to the runtime dir).
        #[arg(long)]
        base: Option<PathBuf>,
    },
```

In `crates/servicio-daemon/src/main.rs`, replace the `Command::Run { .. }` arm with:
```rust
        Command::Serve { base } => {
            use servicio_daemon_lib::lock::InstanceLock;
            use servicio_daemon_lib::paths::Paths;
            use servicio_daemon_lib::serve::serve;
            use servicio_daemon_lib::token::load_or_create;

            let paths = Paths::new(base.unwrap_or_else(Paths::default_base));
            std::fs::create_dir_all(&paths.base)?;
            let _lock = InstanceLock::acquire(&paths.lock())?;
            let token = load_or_create(&paths.token())?;
            let handle = serve(paths, token).await?;
            println!("servicio daemon listening; press Ctrl-C to stop");

            tokio::signal::ctrl_c().await?;
            handle.shutdown().await;
            println!("stopped");
        }
```
(Keep the `add`/`list` arms as-is for now — they are direct-DB conveniences. `--db` global flag still applies to those; `serve` uses `--base` instead.)

- [ ] **Step 7: Full suite + build**

Run: `cargo test` and `cargo build --workspace`
Expected: everything passes and builds. (The `serve` binary path is exercised by integration tests via the library `serve()`.)

- [ ] **Step 8: Commit**

```bash
git add crates/servicio-daemon
git commit -m "feat(daemon): serve command with socket server, token handshake, ping/info, shutdown"
```

---

## Task 6: daemon method dispatch — workers CRUD + control (TDD)

**Files:**
- Modify: `crates/servicio-daemon/src/serve.rs`
- Modify: `crates/servicio-daemon/tests/serve_integration.rs`

- [ ] **Step 1: Write the failing integration test**

Add to `crates/servicio-daemon/tests/serve_integration.rs` (add `use servicio_core::worker::{RestartPolicy, RunMode, WorkerSpec};` and `use std::collections::BTreeMap;` at the top):
```rust
async fn hello_then(sock: &std::path::Path, reqs: Vec<Frame>) -> Vec<Frame> {
    let mut frames = vec![Frame::Request { id: 0, method: "hello".into(), params: json!({"token":"secret"}) }];
    frames.extend(reqs);
    send_recv(sock, &frames).await
}

#[tokio::test]
async fn add_then_list_reflects_worker() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;

    let spec = WorkerSpec {
        name: "q".into(),
        command: "sh".into(),
        args: vec!["-c".into(), "sleep 30".into()],
        working_dir: std::path::PathBuf::from("/"),
        env: BTreeMap::new(),
        run_mode: RunMode::Daemon { concurrency: 1 },
        restart: RestartPolicy::default(),
        autostart: false,
        enabled: true,
    };

    let replies = hello_then(
        &paths.socket(),
        vec![
            Frame::Request { id: 1, method: "add_worker".into(), params: json!({ "spec": spec }) },
            Frame::Request { id: 2, method: "list_workers".into(), params: json!({}) },
        ],
    )
    .await;

    // reply[0] = hello, [1] = add, [2] = list
    match &replies[2] {
        Frame::Response { id: 2, result: Some(v), .. } => {
            let arr = v.as_array().unwrap();
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0]["name"], "q");
        }
        other => panic!("unexpected list reply: {other:?}"),
    }
    h.shutdown().await;
}

#[tokio::test]
async fn start_then_stop_worker() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;

    let spec = WorkerSpec {
        name: "q".into(),
        command: "sh".into(),
        args: vec!["-c".into(), "sleep 30".into()],
        working_dir: std::path::PathBuf::from("/"),
        env: BTreeMap::new(),
        run_mode: RunMode::Daemon { concurrency: 1 },
        restart: RestartPolicy::default(),
        autostart: false,
        enabled: true,
    };

    let replies = hello_then(
        &paths.socket(),
        vec![
            Frame::Request { id: 1, method: "add_worker".into(), params: json!({ "spec": spec }) },
            Frame::Request { id: 2, method: "start_worker".into(), params: json!({"name":"q"}) },
            Frame::Request { id: 3, method: "stop_worker".into(), params: json!({"name":"q"}) },
        ],
    )
    .await;
    assert!(matches!(replies[2], Frame::Response { id: 2, error: None, .. }));
    assert!(matches!(replies[3], Frame::Response { id: 3, error: None, .. }));
    h.shutdown().await;
}
```

- [ ] **Step 2: Run, confirm FAIL**

Run: `cargo test -p servicio-daemon --test serve_integration add_then_list`
Expected: FAIL — list returns `unknown_method`.

- [ ] **Step 3: Extend `dispatch`**

In `crates/servicio-daemon/src/serve.rs`, add these imports near the top:
```rust
use servicio_ipc::types::{InstanceStatus as IpcInstanceStatus, WorkerStatus};
use servicio_core::worker::WorkerSpec;
```
Then extend the `match method` in `dispatch` with these arms (before the `other =>` arm):
```rust
        "list_workers" => {
            let mgr = daemon.manager.lock().await;
            let list: Vec<WorkerStatus> = mgr
                .status()
                .into_iter()
                .map(|w| WorkerStatus {
                    name: w.name,
                    run_mode: w.run_mode,
                    instances: w
                        .instances
                        .into_iter()
                        .map(|i| IpcInstanceStatus {
                            index: i.index,
                            state: i.state,
                            restart_count: i.restart_count,
                            pid: i.pid,
                        })
                        .collect(),
                })
                .collect();
            match serde_json::to_value(list) {
                Ok(v) => Frame::ok(id, v),
                Err(e) => Frame::err(id, "internal", &e.to_string()),
            }
        }
        "add_worker" => {
            let spec: Result<WorkerSpec, _> = serde_json::from_value(_params.get("spec").cloned().unwrap_or(serde_json::Value::Null));
            match spec {
                Ok(spec) => {
                    let name = spec.name.clone();
                    let db = daemon.db.lock().await;
                    match db.upsert_worker(&spec) {
                        Ok(()) => Frame::ok(id, json!({"name": name})),
                        Err(e) => Frame::err(id, "db_error", &e.to_string()),
                    }
                }
                Err(e) => Frame::err(id, "bad_params", &e.to_string()),
            }
        }
        "remove_worker" => {
            let name = _params.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            {
                let mut mgr = daemon.manager.lock().await;
                mgr.stop_worker(&name).await;
            }
            let db = daemon.db.lock().await;
            match db.remove_worker(&name) {
                Ok(removed) => Frame::ok(id, json!({"removed": removed})),
                Err(e) => Frame::err(id, "db_error", &e.to_string()),
            }
        }
        "start_worker" => {
            let name = _params.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            let db = daemon.db.lock().await;
            let spec = db.get_worker(&name);
            drop(db);
            match spec {
                Ok(Some(spec)) => {
                    let mut mgr = daemon.manager.lock().await;
                    mgr.start_worker(spec).await;
                    Frame::ok(id, json!({"started": true}))
                }
                Ok(None) => Frame::err(id, "not_found", &format!("no worker '{name}'")),
                Err(e) => Frame::err(id, "db_error", &e.to_string()),
            }
        }
        "stop_worker" => {
            let name = _params.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            let mut mgr = daemon.manager.lock().await;
            let stopped = mgr.stop_worker(&name).await;
            Frame::ok(id, json!({"stopped": stopped}))
        }
```
Rename the `_params` parameter of `dispatch` to `params` (it is now used), updating the signature to `params: serde_json::Value`.

- [ ] **Step 4: Add `get_worker` + `remove_worker` to `Db`**

In `crates/servicio-daemon/src/db.rs`, add to `impl Db` (above the `query` helper):
```rust
    /// Fetch one worker by name.
    pub fn get_worker(&self, name: &str) -> rusqlite::Result<Option<WorkerSpec>> {
        let mut stmt = self.conn.prepare("SELECT spec_json FROM workers WHERE name = ?1")?;
        let mut rows = stmt.query_map([name], |row| {
            let json: String = row.get(0)?;
            Ok(serde_json::from_str::<WorkerSpec>(&json).expect("stored spec parses"))
        })?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// Delete a worker by name; returns true if a row was removed.
    pub fn remove_worker(&self, name: &str) -> rusqlite::Result<bool> {
        let n = self.conn.execute("DELETE FROM workers WHERE name = ?1", [name])?;
        Ok(n > 0)
    }
```
Add a quick unit test to `db.rs`'s test module:
```rust
    #[test]
    fn get_and_remove_worker() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_worker(&spec("q")).unwrap();
        assert!(db.get_worker("q").unwrap().is_some());
        assert!(db.remove_worker("q").unwrap());
        assert!(db.get_worker("q").unwrap().is_none());
        assert!(!db.remove_worker("q").unwrap());
    }
```

- [ ] **Step 5: Run, confirm PASS**

Run: `cargo test -p servicio-daemon`
Expected: PASS — db unit tests + serve_integration (handshake/ping/shutdown + add/list + start/stop).

- [ ] **Step 6: Commit**

```bash
git add crates/servicio-daemon
git commit -m "feat(daemon): dispatch list/add/remove/start/stop over manager and db"
```

---

## Task 7: daemon `subscribe` — live event fan-out (TDD)

**Files:**
- Modify: `crates/servicio-daemon/src/serve.rs`
- Modify: `crates/servicio-daemon/tests/serve_integration.rs`

- [ ] **Step 1: Write the failing integration test**

Add to `crates/servicio-daemon/tests/serve_integration.rs`:
```rust
#[tokio::test]
async fn subscribe_streams_state_events_for_started_worker() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;

    // First connection: register + start the worker.
    let spec = WorkerSpec {
        name: "q".into(),
        command: "sh".into(),
        args: vec!["-c".into(), "sleep 30".into()],
        working_dir: std::path::PathBuf::from("/"),
        env: BTreeMap::new(),
        run_mode: RunMode::Daemon { concurrency: 1 },
        restart: RestartPolicy::default(),
        autostart: false,
        enabled: true,
    };
    let _ = hello_then(
        &paths.socket(),
        vec![Frame::Request { id: 1, method: "add_worker".into(), params: json!({ "spec": spec }) }],
    )
    .await;

    // Second connection: subscribe, THEN trigger a start, and read events.
    let stream = UnixStream::connect(&paths.socket()).await.unwrap();
    let (rd, mut wr) = stream.into_split();
    for f in [
        Frame::Request { id: 0, method: "hello".into(), params: json!({"token":"secret"}) },
        Frame::Request { id: 1, method: "subscribe".into(), params: json!({"topics":["state"]}) },
    ] {
        wr.write_all(format!("{}\n", f.to_line()).as_bytes()).await.unwrap();
    }
    let mut lines = BufReader::new(rd).lines();
    // consume hello reply + subscribe reply
    let _ = lines.next_line().await.unwrap();
    let _ = lines.next_line().await.unwrap();

    // Trigger a start on a separate connection.
    let _ = hello_then(
        &paths.socket(),
        vec![Frame::Request { id: 1, method: "start_worker".into(), params: json!({"name":"q"}) }],
    )
    .await;

    // Expect a state Event within a couple seconds.
    let got = tokio::time::timeout(Duration::from_secs(3), async {
        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(Frame::Event { topic, .. }) = Frame::from_line(&line) {
                if topic == "state" { return true; }
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(got, "expected a state event after start");
    h.shutdown().await;
}
```

- [ ] **Step 2: Run, confirm FAIL**

Run: `cargo test -p servicio-daemon --test serve_integration subscribe_streams`
Expected: FAIL — subscribe returns `unknown_method` and no events arrive.

- [ ] **Step 3: Implement subscribe + event forwarding**

In `crates/servicio-daemon/src/serve.rs`:

(a) Add imports:
```rust
use servicio_core::event::SupervisorEvent;
use servicio_ipc::types::{LogEvent, StateEvent};
```

(b) `handle_conn` must be able to forward events concurrently with reading requests. Replace the body of `handle_conn` with a version that, on a `subscribe` request, spawns a forwarding task. Change the `dispatch` call site so `subscribe` is handled inline (it needs the write half + a broadcast receiver):
```rust
async fn handle_conn(stream: UnixStream, daemon: Arc<Daemon>) {
    let (rd, wr) = stream.into_split();
    let wr = Arc::new(Mutex::new(wr));
    let mut lines = BufReader::new(rd).lines();
    let mut authed = false;

    while let Ok(Some(line)) = lines.next_line().await {
        let frame = match Frame::from_line(&line) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let Frame::Request { id, method, params } = frame else { continue };

        if !authed {
            if method == "hello"
                && params.get("token").and_then(|t| t.as_str()) == Some(daemon.token.as_str())
            {
                authed = true;
                let _ = write_frame_locked(&wr, &Frame::ok(id, json!({"daemon_version": daemon.version}))).await;
                continue;
            }
            let _ = write_frame_locked(&wr, &Frame::err(id, "unauthorized", "valid hello required")).await;
            return;
        }

        if method == "subscribe" {
            let topics: Vec<String> = params
                .get("topics")
                .and_then(|t| t.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let worker_filter = params.get("worker").and_then(|w| w.as_str()).map(String::from);
            let rx = daemon.manager.lock().await.subscribe();
            let _ = write_frame_locked(&wr, &Frame::ok(id, json!({"subscribed": true}))).await;
            spawn_forwarder(Arc::clone(&wr), rx, topics, worker_filter);
            continue;
        }

        let reply = dispatch(&daemon, id, &method, params).await;
        if write_frame_locked(&wr, &reply).await.is_err() {
            return;
        }
    }
}

fn spawn_forwarder(
    wr: Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
    mut rx: tokio::sync::broadcast::Receiver<SupervisorEvent>,
    topics: Vec<String>,
    worker_filter: Option<String>,
) {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let frame = match ev {
                        SupervisorEvent::State { worker, instance, from, to } => {
                            if !topics.iter().any(|t| t == "state") { continue; }
                            if let Some(f) = &worker_filter { if f != &worker { continue; } }
                            Frame::Event {
                                topic: "state".into(),
                                payload: serde_json::to_value(StateEvent { worker, instance, from, to }).unwrap(),
                            }
                        }
                        SupervisorEvent::Log { worker, instance, stream, line } => {
                            if !topics.iter().any(|t| t == "log") { continue; }
                            if let Some(f) = &worker_filter { if f != &worker { continue; } }
                            Frame::Event {
                                topic: "log".into(),
                                payload: serde_json::to_value(LogEvent { worker, instance, stream, line }).unwrap(),
                            }
                        }
                    };
                    if write_frame_locked(&wr, &frame).await.is_err() { break; }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    let frame = Frame::Event { topic: "lagged".into(), payload: json!({"dropped": n}) };
                    if write_frame_locked(&wr, &frame).await.is_err() { break; }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

async fn write_frame_locked(
    wr: &Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
    frame: &Frame,
) -> std::io::Result<()> {
    let mut guard = wr.lock().await;
    guard.write_all(format!("{}\n", frame.to_line()).as_bytes()).await
}
```

(c) Remove the now-unused old `write_frame` free function (replaced by `write_frame_locked`). If `dispatch` is unaffected (it returns a `Frame`), no other change needed.

- [ ] **Step 4: Run, confirm PASS**

Run: `cargo test -p servicio-daemon --test serve_integration`
Expected: PASS — all serve integration tests including `subscribe_streams_state_events_for_started_worker`.

- [ ] **Step 5: Full suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/servicio-daemon
git commit -m "feat(daemon): subscribe streams state/log events with lag handling"
```

---

## Task 8: `servicio-cli` client (TDD via end-to-end)

**Files:**
- Modify: root `Cargo.toml` (add member)
- Create: `crates/servicio-cli/Cargo.toml`, `src/main.rs`, `src/client.rs`
- Create: `crates/servicio-cli/tests/e2e.rs`

- [ ] **Step 1: Add the crate to the workspace**

Edit root `Cargo.toml`:
```toml
members = ["crates/servicio-core", "crates/servicio-daemon", "crates/servicio-ipc", "crates/servicio-cli"]
```

- [ ] **Step 2: Crate manifest**

Create `crates/servicio-cli/Cargo.toml`:
```toml
[package]
name = "servicio-cli"
edition.workspace = true
version.workspace = true
license.workspace = true

[[bin]]
name = "servicio"
path = "src/main.rs"

[lib]
name = "servicio_cli_lib"
path = "src/client.rs"

[dependencies]
servicio-ipc = { path = "../servicio-ipc" }
tokio.workspace = true
serde_json.workspace = true
clap.workspace = true
anyhow.workspace = true

[dev-dependencies]
tempfile.workspace = true
servicio-daemon = { path = "../servicio-daemon" }
servicio-core = { path = "../servicio-core" }
```

- [ ] **Step 3: Write the failing end-to-end test**

Create `crates/servicio-cli/tests/e2e.rs`:
```rust
// Drive the real client against an in-process daemon over a temp socket.
use servicio_cli_lib::Client;
use servicio_daemon_lib::paths::Paths;
use servicio_daemon_lib::serve::serve;
use servicio_core::worker::{RestartPolicy, RunMode, WorkerSpec};
use std::collections::BTreeMap;
use std::time::Duration;

#[tokio::test]
async fn client_handshakes_and_lists_after_add() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let handle = serve(paths.clone(), "secret".into()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut client = Client::connect(&paths.socket(), "secret").await.unwrap();

    let spec = WorkerSpec {
        name: "q".into(),
        command: "sh".into(),
        args: vec!["-c".into(), "sleep 30".into()],
        working_dir: std::path::PathBuf::from("/"),
        env: BTreeMap::new(),
        run_mode: RunMode::Daemon { concurrency: 1 },
        restart: RestartPolicy::default(),
        autostart: false,
        enabled: true,
    };
    client.add_worker(&spec).await.unwrap();
    let list = client.list_workers().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "q");

    handle.shutdown().await;
}
```

- [ ] **Step 4: Run, confirm FAIL**

Run: `cargo test -p servicio-cli --test e2e`
Expected: FAIL — unresolved `servicio_cli_lib::Client`.

- [ ] **Step 5: Implement the client library**

Create `crates/servicio-cli/src/client.rs`:
```rust
//! Thin async client for the servicio daemon.

use anyhow::{anyhow, Result};
use servicio_ipc::types::WorkerStatus;
use servicio_ipc::Frame;
use serde_json::{json, Value};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;

pub struct Client {
    wr: OwnedWriteHalf,
    lines: Lines<BufReader<OwnedReadHalf>>,
    next_id: u64,
}

impl Client {
    /// Connect and perform the `hello` handshake.
    pub async fn connect(socket: &Path, token: &str) -> Result<Self> {
        let stream = UnixStream::connect(socket).await?;
        let (rd, wr) = stream.into_split();
        let lines = BufReader::new(rd).lines();
        let mut c = Client { wr, lines, next_id: 1 };
        let res = c.request("hello", json!({ "token": token })).await?;
        let _ = res; // hello result is daemon_version; ignore
        Ok(c)
    }

    /// Send a request and await its matching response (no interleaved events expected
    /// on the control path; subscribe uses `into_event_stream`).
    pub async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let frame = Frame::Request { id, method: method.into(), params };
        self.wr.write_all(format!("{}\n", frame.to_line()).as_bytes()).await?;
        loop {
            let line = self
                .lines
                .next_line()
                .await?
                .ok_or_else(|| anyhow!("connection closed"))?;
            match Frame::from_line(&line)? {
                Frame::Response { id: rid, result, error } if rid == id => {
                    if let Some(e) = error {
                        return Err(anyhow!("{}: {}", e.code, e.message));
                    }
                    return Ok(result.unwrap_or(Value::Null));
                }
                _ => continue, // skip events/other ids
            }
        }
    }

    pub async fn ping(&mut self) -> Result<()> {
        self.request("ping", json!({})).await.map(|_| ())
    }

    pub async fn add_worker(&mut self, spec: &servicio_core::worker::WorkerSpec) -> Result<()> {
        self.request("add_worker", json!({ "spec": spec })).await.map(|_| ())
    }

    pub async fn list_workers(&mut self) -> Result<Vec<WorkerStatus>> {
        let v = self.request("list_workers", json!({})).await?;
        Ok(serde_json::from_value(v)?)
    }

    pub async fn start_worker(&mut self, name: &str) -> Result<()> {
        self.request("start_worker", json!({ "name": name })).await.map(|_| ())
    }

    pub async fn stop_worker(&mut self, name: &str) -> Result<()> {
        self.request("stop_worker", json!({ "name": name })).await.map(|_| ())
    }

    pub async fn daemon_info(&mut self) -> Result<Value> {
        self.request("daemon_info", json!({})).await
    }

    /// Send a subscribe request, then yield raw event lines. Consumes self.
    pub async fn subscribe(
        mut self,
        topics: &[&str],
        worker: Option<&str>,
    ) -> Result<Lines<BufReader<OwnedReadHalf>>> {
        let id = self.next_id;
        let params = json!({ "topics": topics, "worker": worker });
        let frame = Frame::Request { id, method: "subscribe".into(), params };
        self.wr.write_all(format!("{}\n", frame.to_line()).as_bytes()).await?;
        // Wait for the subscribe ack, then hand back the line stream for events.
        loop {
            let line = self.lines.next_line().await?.ok_or_else(|| anyhow!("closed"))?;
            if let Frame::Response { id: rid, .. } = Frame::from_line(&line)? {
                if rid == id { break; }
            }
        }
        Ok(self.lines)
    }
}
```

- [ ] **Step 6: Run, confirm PASS**

Run: `cargo test -p servicio-cli --test e2e`
Expected: PASS — 1 test.

- [ ] **Step 7: Implement the `servicio` binary**

Create `crates/servicio-cli/src/main.rs`:
```rust
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use servicio_cli_lib::Client;
use servicio_ipc::Frame;
use std::path::PathBuf;
use tokio::io::AsyncBufReadExt;

#[derive(Parser)]
#[command(name = "servicio", about = "Control the servicio daemon")]
struct Cli {
    /// Base dir where the daemon's socket + token live.
    #[arg(long)]
    base: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List workers and their status.
    Ps,
    /// Daemon info.
    Info,
    /// Start a worker.
    Start { name: String },
    /// Stop a worker.
    Stop { name: String },
    /// Stream logs for a worker (follow until Ctrl-C).
    Logs { name: String },
}

fn base_dir(arg: Option<PathBuf>) -> PathBuf {
    // Mirror the daemon's default base resolution.
    arg.unwrap_or_else(|| {
        if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
            PathBuf::from(dir).join("servicio")
        } else {
            std::env::temp_dir().join("servicio")
        }
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let base = base_dir(cli.base);
    let socket = base.join("daemon.sock");
    let token = std::fs::read_to_string(base.join("token"))
        .context("reading token (is the daemon running?)")?
        .trim()
        .to_string();

    let mut client = Client::connect(&socket, &token).await.context("connecting to daemon")?;

    match cli.command {
        Command::Ps => {
            let workers = client.list_workers().await?;
            println!("{:<20} {:<22} {:<10} RESTARTS", "NAME", "MODE", "STATE");
            for w in workers {
                let state = w.instances.first().map(|i| format!("{:?}", i.state)).unwrap_or_else(|| "-".into());
                let restarts: u32 = w.instances.iter().map(|i| i.restart_count).sum();
                println!("{:<20} {:<22} {:<10} {}", w.name, format!("{:?}", w.run_mode), state, restarts);
            }
        }
        Command::Info => {
            let info = client.daemon_info().await?;
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        Command::Start { name } => {
            client.start_worker(&name).await?;
            println!("started '{name}'");
        }
        Command::Stop { name } => {
            client.stop_worker(&name).await?;
            println!("stopped '{name}'");
        }
        Command::Logs { name } => {
            let mut lines = client.subscribe(&["log"], Some(&name)).await?;
            println!("following logs for '{name}' (Ctrl-C to stop)");
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(Frame::Event { topic, payload }) = Frame::from_line(&line) {
                    if topic == "log" {
                        let l = payload.get("line").and_then(|v| v.as_str()).unwrap_or("");
                        let stream = payload.get("stream").and_then(|v| v.as_str()).unwrap_or("");
                        println!("[{stream}] {l}");
                    }
                }
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 8: Build + full suite**

Run: `cargo build --workspace && cargo test`
Expected: builds; all tests pass.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml Cargo.lock crates/servicio-cli
git commit -m "feat(cli): servicio client binary + library (ps/info/start/stop/logs)"
```

---

## Task 9: README update + manual smoke

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update the README "Try it" section**

Replace the "Try it" code block in `README.md` with:
```markdown
### Try it (Phase 2a)

Start the daemon in one terminal:

```bash
cargo run -p servicio-daemon -- serve --base /tmp/servicio
```

In another terminal, drive it with the `servicio` CLI:

```bash
# (worker registration via the Phase-1 add still uses the daemon's add subcommand against the same db;
#  the canonical control path is the client below)
cargo run -p servicio-cli -- --base /tmp/servicio ps
cargo run -p servicio-cli -- --base /tmp/servicio info
cargo run -p servicio-cli -- --base /tmp/servicio start ticker
cargo run -p servicio-cli -- --base /tmp/servicio logs ticker
```
```
Also update the Crates list to mention `servicio-ipc` (protocol) and `servicio-cli` (client).

- [ ] **Step 2: Manual smoke (document result, do not automate)**

Run the daemon with `--base /tmp/servicio-smoke serve` in the background, then
`cargo run -p servicio-cli -- --base /tmp/servicio-smoke info` and confirm JSON prints.
Stop the daemon (Ctrl-C / kill) and confirm the socket file is gone.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: phase 2a daemon serve + servicio client usage"
```

---

## Definition of Done (Phase 2a)
- `cargo test` green across the workspace, including ipc unit tests, core event/status/state-machine tests, daemon `serve_integration`, and the cli `e2e` test.
- Daemon `serve` binds a `0600` Unix socket, requires a token `hello`, dispatches
  list/add/remove/start/stop/info, and streams state+log events on `subscribe`.
- Single-instance lock prevents a second daemon; graceful shutdown removes socket + lock.
- State machine is wired into the supervisor (transitions validated + emitted).
- `servicio` CLI drives the daemon end-to-end (`ps/info/start/stop/logs`).
- `servicio-ipc` has no tokio dependency (pure protocol crate).

## Out of scope (later)
- Tauri GUI (Phase 2b).
- Windows named-pipe transport (trait-gate + impl when Windows is exercised).
- OS-service install / run-on-boot.
- Scheduled/batch modes, metrics history, notifications, remote/TLS.

## Self-review notes
- **Spec coverage:** ipc crate (§4) → Tasks 0–1; core events/status/state-machine (§5) →
  Tasks 2–3; serve/lock/auth/shutdown (§5) → Tasks 4–5; method dispatch (§4) → Task 6;
  subscribe/events (§4) → Task 7; CLI (§6) → Task 8; testing (§8) → embedded per task;
  docs → Task 9. Security (§7): socket `0600`, token `0600`, token-gated `hello` → Tasks 4–5.
- **Type consistency:** `Frame`/`ApiError` (Task 0); ipc `WorkerStatus`/`InstanceStatus`/
  `StateEvent`/`LogEvent` (Task 1) mapped from core `WorkerStatusCore`/`InstanceStatus`
  (Task 2) in the daemon (Task 6); `SupervisorEvent` (Task 2) consumed by the forwarder
  (Task 7); `Manager::{subscribe,status,start_worker,stop_worker,stop_all,instance_count}`,
  `InstanceSupervisor::{with_events,state,pid,index,restart_count,worker_name}`,
  `Db::{get_worker,remove_worker,upsert_worker,autostart_workers}`, `Paths`, `token::load_or_create`,
  `InstanceLock::acquire`, `serve`/`ServeHandle::shutdown`, `Client::*` are referenced consistently.
- **Note on the daemon `add`/`list` subcommands:** kept as Phase-1 direct-DB conveniences;
  the canonical control path is now CLI→IPC. Removing them is optional and deferred.
