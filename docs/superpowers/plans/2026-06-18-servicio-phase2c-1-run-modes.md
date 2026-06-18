# Servicio Phase 2c.1 — Engine Run Modes (Scheduled + Batch) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `Scheduled` (cron/interval) and `Batch` (run-N-times) run modes to the `servicio-core` engine alongside the existing `Daemon` mode, with a shared "run one instance" core and mode-specific outer loops, fully TDD'd.

**Architecture:** Extend `RunMode` with two variants + supporting enums (`Schedule`, `OverlapPolicy`). Add `Idle` and `Completed` states to the lifecycle machine. Refactor `InstanceSupervisor` so a private `run_once()` (spawn → pump logs → wait → outcome) is shared by three outer loops dispatched from `run_until_terminal` based on the worker's run mode. `Manager` spawns N instances for daemon, 1 for scheduled/batch. Cron next-fire is a pure, separately-tested calculation.

**Tech Stack:** Rust, Tokio, `cron` + `chrono` (cron parsing/next-fire), serde. Tests: `#[tokio::test]`, `tempfile`, real cheap `sh` processes; scheduled tests use short `IntervalSecs` so they fire fast.

**Builds on:** Phases 1–2b (merged). Spec: `docs/superpowers/specs/2026-06-18-servicio-phase2c-design.md`.

---

## File Structure
```
crates/servicio-core/
  Cargo.toml          # + cron, chrono deps
  src/
    worker.rs         # RunMode + Schedule + OverlapPolicy variants
    state.rs          # add Idle, Completed states + transitions
    schedule.rs       # NEW: pure cron/interval next-fire calc
    supervisor.rs     # run_once() + daemon/scheduled/batch loops
    manager.rs        # instance count per mode
    lib.rs            # pub mod schedule;
```

---

## Task 1: RunMode variants + Schedule/OverlapPolicy (TDD)

**Files:** `crates/servicio-core/src/worker.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `worker.rs`:
```rust
    #[test]
    fn scheduled_mode_roundtrips() {
        let m = RunMode::Scheduled {
            schedule: Schedule::Cron("0 3 * * *".into()),
            overlap: OverlapPolicy::Skip,
        };
        let back: RunMode = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn batch_mode_roundtrips() {
        let m = RunMode::Batch { run_count: 5, delay_secs: 10 };
        let back: RunMode = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn interval_schedule_roundtrips() {
        let s = Schedule::IntervalSecs(30);
        let back: Schedule = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(s, back);
    }
```

- [ ] **Step 2: Run, confirm FAIL**

Run: `cargo test -p servicio-core scheduled_mode_roundtrips`
Expected: FAIL — `Schedule`/`OverlapPolicy`/variants not found.

- [ ] **Step 3: Implement**

In `worker.rs`, add above `RunMode` and extend the enum:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Schedule {
    Cron(String),
    IntervalSecs(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlapPolicy {
    Skip,
    Queue,
    KillPrevious,
}
```
Replace the `RunMode` enum with:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunMode {
    Daemon {
        #[serde(default = "default_concurrency")]
        concurrency: u32,
    },
    Scheduled {
        schedule: Schedule,
        #[serde(default = "default_overlap")]
        overlap: OverlapPolicy,
    },
    Batch {
        run_count: u32,
        #[serde(default)]
        delay_secs: u64,
    },
}

fn default_overlap() -> OverlapPolicy { OverlapPolicy::Skip }
```
(`default_concurrency` already exists.)

- [ ] **Step 4: Run, confirm PASS** — `cargo test -p servicio-core worker` (existing + 3 new pass). The exhaustive `match spec.run_mode` in `manager.rs` will now FAIL to compile — that is expected and fixed in Task 5; for now, to keep this task's tests runnable, add a temporary catch in manager.rs Step noted below. Actually: to avoid a broken build between tasks, in `manager.rs`'s `start_worker` change the `match` to compute concurrency with a wildcard now:
```rust
        let concurrency = match spec.run_mode {
            RunMode::Daemon { concurrency } => concurrency.max(1),
            _ => 1, // scheduled/batch get one supervisor; refined in Task 5
        };
```

- [ ] **Step 5: Verify whole crate compiles + tests pass**

Run: `cargo test -p servicio-core`
Expected: all pass.

- [ ] **Step 6: Commit**
```bash
git add crates/servicio-core/src/worker.rs crates/servicio-core/src/manager.rs
git commit -m "feat(core): Scheduled + Batch run mode variants"
```

---

## Task 2: Lifecycle states Idle + Completed (TDD)

**Files:** `crates/servicio-core/src/state.rs`

- [ ] **Step 1: Failing tests**

Add to `state.rs` tests:
```rust
    #[test]
    fn scheduled_idle_run_cycle_is_legal() {
        assert!(InstanceState::Running.can_transition_to(InstanceState::Idle));
        assert!(InstanceState::Idle.can_transition_to(InstanceState::Starting));
    }

    #[test]
    fn batch_completion_is_terminal() {
        assert!(InstanceState::Running.can_transition_to(InstanceState::Completed));
        assert!(InstanceState::Completed.is_terminal());
    }
```

- [ ] **Step 2: Run, confirm FAIL** — `cargo test -p servicio-core scheduled_idle` → `Idle`/`Completed` not found.

- [ ] **Step 3: Implement**

In `state.rs`, add `Idle` and `Completed` to the `InstanceState` enum (after `Backoff`):
```rust
    Idle,
    Completed,
```
Add to `is_terminal`:
```rust
        matches!(self, InstanceState::Stopped | InstanceState::Failed | InstanceState::Completed)
```
Add these arms to `can_transition_to`'s `matches!`:
```rust
                | (Running, Idle)
                | (Idle, Starting)
                | (Idle, Stopped)
                | (Running, Completed)
                | (Starting, Completed)
```

- [ ] **Step 4: Run, confirm PASS** — `cargo test -p servicio-core state`.

- [ ] **Step 5: Commit**
```bash
git add crates/servicio-core/src/state.rs
git commit -m "feat(core): Idle + Completed lifecycle states"
```

---

## Task 3: Pure schedule next-fire calc (TDD)

**Files:** `crates/servicio-core/Cargo.toml`, `crates/servicio-core/src/schedule.rs`, `lib.rs`

- [ ] **Step 1: Add deps**

In `crates/servicio-core/Cargo.toml` `[dependencies]`:
```toml
cron = "0.12"
chrono = { version = "0.4", default-features = false, features = ["clock"] }
```

- [ ] **Step 2: Failing tests**

Create `crates/servicio-core/src/schedule.rs`:
```rust
use crate::worker::Schedule;
use std::time::Duration;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn interval_delay_is_constant() {
        let d = next_delay(&Schedule::IntervalSecs(30), Utc::now()).unwrap();
        assert_eq!(d, Duration::from_secs(30));
    }

    #[test]
    fn cron_daily_3am_next_delay_is_positive_and_bounded() {
        // from just after midnight, next 3am is < 3h away.
        let from = Utc.with_ymd_and_hms(2026, 1, 1, 0, 30, 0).unwrap();
        let d = next_delay(&Schedule::Cron("0 3 * * *".into()), from).unwrap();
        assert_eq!(d, Duration::from_secs((2 * 60 + 30) * 60));
    }

    #[test]
    fn invalid_cron_is_error() {
        assert!(next_delay(&Schedule::Cron("not a cron".into()), Utc::now()).is_err());
    }
}
```

- [ ] **Step 3: Run, confirm FAIL** — `cargo test -p servicio-core schedule` → `next_delay` not found.

- [ ] **Step 4: Implement**

Add above the test block in `schedule.rs`:
```rust
use crate::error::CoreError;
use chrono::{DateTime, Utc};
use std::str::FromStr;

/// Delay from `from` until the schedule's next fire.
pub fn next_delay(schedule: &Schedule, from: DateTime<Utc>) -> Result<Duration, CoreError> {
    match schedule {
        Schedule::IntervalSecs(n) => Ok(Duration::from_secs(*n)),
        Schedule::Cron(expr) => {
            // The `cron` crate expects a 6 or 7 field expression (with seconds);
            // accept the common 5-field form by prefixing a "0 " seconds field.
            let normalized = normalize_cron(expr);
            let sched = cron::Schedule::from_str(&normalized)
                .map_err(|e| CoreError::Spawn(format!("invalid cron '{expr}': {e}")))?;
            let next = sched
                .after(&from)
                .next()
                .ok_or_else(|| CoreError::Spawn(format!("cron '{expr}' has no next time")))?;
            let secs = (next - from).num_seconds().max(0) as u64;
            Ok(Duration::from_secs(secs))
        }
    }
}

/// Turn a 5-field cron expr into the 6-field (seconds-leading) form the crate wants.
fn normalize_cron(expr: &str) -> String {
    let fields = expr.split_whitespace().count();
    if fields == 5 {
        format!("0 {expr}")
    } else {
        expr.to_string()
    }
}
```
Add `pub mod schedule;` to `lib.rs` (alphabetical, after `pub mod process;`... place near other modules).

- [ ] **Step 5: Run, confirm PASS** — `cargo test -p servicio-core schedule`. If the `cron` crate rejects 5-field even with the seconds prefix, report the exact parse error (the crate may want 6 fields without DOW/year — adjust `normalize_cron` accordingly but keep the daily-3am test semantics).

- [ ] **Step 6: Commit**
```bash
git add crates/servicio-core/src/schedule.rs crates/servicio-core/src/lib.rs crates/servicio-core/Cargo.toml Cargo.lock
git commit -m "feat(core): pure cron/interval next-fire calculation"
```

---

## Task 4: Supervisor run_once + scheduled/batch loops (TDD)

**Files:** `crates/servicio-core/src/supervisor.rs`

This refactors the supervisor so daemon/scheduled/batch share one "run an instance once" core. Read the current `supervisor.rs` fully before editing — preserve the existing event/pid/logsink/backoff behaviour.

- [ ] **Step 1: Failing tests**

Add to `supervisor.rs` tests:
```rust
    #[tokio::test]
    async fn batch_runs_exactly_n_times_then_completed() {
        let dir = tempfile::tempdir().unwrap();
        let counter = dir.path().join("count");
        let mut s = spec("sh", &["-c", &format!("echo x >> {}", counter.display())],
                         RestartPolicy { kind: RestartKind::Never, ..Default::default() });
        s.run_mode = RunMode::Batch { run_count: 3, delay_secs: 0 };
        s.working_dir = dir.path().to_path_buf();
        let sup = InstanceSupervisor::new(0, s, Arc::new(TokioProcess), dir.path().join("b.log"));
        let mut rx = sup.subscribe();
        sup.run_until_terminal().await;
        assert_eq!(std::fs::read_to_string(&counter).unwrap().lines().count(), 3);
        assert_eq!(*rx.borrow_and_update(), InstanceState::Completed);
    }

    #[tokio::test]
    async fn scheduled_interval_fires_multiple_times() {
        let dir = tempfile::tempdir().unwrap();
        let counter = dir.path().join("count");
        let mut s = spec("sh", &["-c", &format!("echo x >> {}", counter.display())],
                         RestartPolicy { kind: RestartKind::Never, ..Default::default() });
        s.run_mode = RunMode::Scheduled {
            schedule: crate::worker::Schedule::IntervalSecs(1),
            overlap: crate::worker::OverlapPolicy::Skip,
        };
        s.working_dir = dir.path().to_path_buf();
        let sup = std::sync::Arc::new(InstanceSupervisor::new(0, s, Arc::new(TokioProcess), dir.path().join("s.log")));
        let run = sup.clone();
        let h = tokio::spawn(async move { run.run_until_terminal().await });
        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
        h.abort();
        let n = std::fs::read_to_string(&counter).map(|c| c.lines().count()).unwrap_or(0);
        assert!(n >= 2, "expected >= 2 fires, got {n}");
    }
```

- [ ] **Step 2: Run, confirm FAIL** — these compile-fail/behave-wrong until `run_until_terminal` handles the new modes.

- [ ] **Step 3: Implement**

In `supervisor.rs`:

(a) Import the new types: `use crate::worker::{RunMode, Schedule, OverlapPolicy};` (extend the existing worker import).

(b) Extract the existing daemon body into `run_daemon(&self)` — rename the current `run_until_terminal`'s body into a private `async fn run_daemon(&self)` (keep every line: backoff, crash-loop, pump, pid, events).

(c) Add a shared single-run helper used by scheduled + batch. Factor the spawn→pump→wait section into:
```rust
    /// Spawn one instance, pump logs concurrently, wait for exit. Returns success.
    async fn run_once(&self, sink: &std::sync::Arc<std::sync::Mutex<LogSink>>) -> bool {
        self.set_state(InstanceState::Starting);
        let mut spawned = match self.spawner.spawn(&self.spec) {
            Ok(s) => s,
            Err(_) => { self.set_state(InstanceState::Crashed); return false; }
        };
        self.set_state(InstanceState::Running);
        self.pid.store(spawned.pid().unwrap_or(0), Ordering::SeqCst);
        let idx = self.index;
        let out = spawned.stdout.take();
        let err = spawned.stderr.take();
        let so = std::sync::Arc::clone(sink);
        let se = std::sync::Arc::clone(sink);
        let ev = self.events.clone();
        let name = self.spec.name.clone();
        let mut pump = tokio::spawn(async move {
            let oev = ev.clone(); let on = name.clone();
            let out_fut = async move {
                if let Some(o) = out {
                    let mut lines = BufReader::new(o).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = so.lock().unwrap().write_line(idx, "stdout", &line);
                        if let Some(tx) = &oev { let _ = tx.send(SupervisorEvent::Log { worker: on.clone(), instance: idx, stream: "stdout".into(), line: line.clone() }); }
                    }
                }
            };
            let eev = ev.clone(); let en = name.clone();
            let err_fut = async move {
                if let Some(e) = err {
                    let mut lines = BufReader::new(e).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = se.lock().unwrap().write_line(idx, "stderr", &line);
                        if let Some(tx) = &eev { let _ = tx.send(SupervisorEvent::Log { worker: en.clone(), instance: idx, stream: "stderr".into(), line: line.clone() }); }
                    }
                }
            };
            tokio::join!(out_fut, err_fut);
        });
        let status = spawned.wait().await;
        if tokio::time::timeout(Duration::from_secs(2), &mut pump).await.is_err() { pump.abort(); }
        self.pid.store(0, Ordering::SeqCst);
        status.map(|s| s.success()).unwrap_or(false)
    }
```
Refactor `run_daemon` to call `run_once` instead of its own inline spawn/pump (DRY) — the daemon loop becomes: `run_once` → reset/backoff/crash-loop bookkeeping as before. Keep the existing daemon tests green.

(d) Add the scheduled + batch loops:
```rust
    async fn run_scheduled(&self, schedule: &Schedule, overlap: OverlapPolicy) {
        let sink = std::sync::Arc::new(std::sync::Mutex::new(
            LogSink::new(&self.log_path, 10 * 1024 * 1024, 5).expect("log sink")));
        loop {
            self.set_state(InstanceState::Idle);
            let delay = match crate::schedule::next_delay(schedule, chrono::Utc::now()) {
                Ok(d) => d,
                Err(_) => { self.set_state(InstanceState::Failed); return; }
            };
            tokio::time::sleep(delay).await;
            // Overlap is naturally handled here for Skip (we await each run before the next).
            // Queue/KillPrevious refinements are noted for a follow-up; Skip is the v1 default.
            let _ = overlap;
            self.run_once(&sink).await;
        }
    }

    async fn run_batch(&self, run_count: u32, delay_secs: u64) {
        let sink = std::sync::Arc::new(std::sync::Mutex::new(
            LogSink::new(&self.log_path, 10 * 1024 * 1024, 5).expect("log sink")));
        let mut any_failed = false;
        for i in 0..run_count {
            let ok = self.run_once(&sink).await;
            if !ok { any_failed = true; }
            if i + 1 < run_count && delay_secs > 0 {
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
            }
        }
        self.set_state(if any_failed { InstanceState::Failed } else { InstanceState::Completed });
    }
```
> Note: v1 scheduled overlap = Skip semantics (each run is awaited before scheduling the next). `Queue`/`KillPrevious` are accepted in the type but behave as Skip for now; a follow-up adds concurrent-run handling. Document this in-code.

(e) Replace `run_until_terminal` to dispatch:
```rust
    pub async fn run_until_terminal(&self) {
        match self.spec.run_mode.clone() {
            RunMode::Daemon { .. } => self.run_daemon().await,
            RunMode::Scheduled { schedule, overlap } => self.run_scheduled(&schedule, overlap).await,
            RunMode::Batch { run_count, delay_secs } => self.run_batch(run_count, delay_secs).await,
        }
    }
```

- [ ] **Step 4: Run, confirm PASS** — `cargo test -p servicio-core supervisor` (existing daemon tests + batch + scheduled). The `scheduled_interval_fires_multiple_times` test waits 2.5s.

- [ ] **Step 5: Full core suite** — `cargo test -p servicio-core` all green.

- [ ] **Step 6: Commit**
```bash
git add crates/servicio-core/src/supervisor.rs
git commit -m "feat(core): run_once core + scheduled and batch supervisor loops"
```

---

## Task 5: Manager mode-aware instance count (TDD)

**Files:** `crates/servicio-core/src/manager.rs`

- [ ] **Step 1: Failing test**

Add to `manager.rs` tests:
```rust
    #[tokio::test]
    async fn batch_worker_starts_single_instance() {
        let dir = tempfile::tempdir().unwrap();
        let mut mgr = Manager::new(Arc::new(TokioProcess), dir.path().to_path_buf());
        let mut s = long_running("b");
        s.run_mode = RunMode::Batch { run_count: 2, delay_secs: 0 };
        s.command = "sh".into();
        s.args = vec!["-c".into(), "true".into()];
        mgr.start_worker(s).await;
        assert_eq!(mgr.instance_count("b"), 1);
        mgr.stop_all().await;
    }
```

- [ ] **Step 2: Implement**

In `manager.rs` `start_worker`, replace the concurrency computation with mode-aware logic:
```rust
        let concurrency = match spec.run_mode {
            RunMode::Daemon { concurrency } => concurrency.max(1),
            RunMode::Scheduled { .. } | RunMode::Batch { .. } => 1,
        };
```
(Remove the temporary wildcard from Task 1.) Ensure `use crate::worker::RunMode;` covers the new variants (it does).

- [ ] **Step 3: Run, confirm PASS** — `cargo test -p servicio-core manager` and full `cargo test -p servicio-core`.

- [ ] **Step 4: Commit**
```bash
git add crates/servicio-core/src/manager.rs
git commit -m "feat(core): manager starts one instance for scheduled/batch workers"
```

---

## Task 6: Workspace build + downstream compile check

**Files:** (none — verification)

Adding `RunMode` variants can break exhaustive matches in `servicio-daemon`/`servicio-ipc`/`servicio-cli`.

- [ ] **Step 1: Build the whole workspace**

Run: `cargo build --workspace`
Expected: compiles. If any crate has a non-exhaustive `match run_mode`, fix it minimally:
- `servicio-ipc/types.rs` uses `RunMode` only as a serde field (no match) — OK.
- `servicio-daemon` maps `WorkerStatusCore.run_mode` into ipc via serde — no match — OK.
- If a `match` appears, add arms for `Scheduled`/`Batch` (e.g. display strings).

- [ ] **Step 2: Full workspace test**

Run: `cargo test`
Expected: all green (engine new tests + existing 54).

- [ ] **Step 3: Commit (only if Step 1 required edits)**
```bash
git add -A
git commit -m "fix: handle new run-mode variants in downstream crates"
```
(If nothing needed changing, skip the commit.)

---

## Definition of Done (2c.1)
- `RunMode` has `Daemon`/`Scheduled`/`Batch`; `Schedule` + `OverlapPolicy` serde-roundtrip.
- `InstanceState` has `Idle` + `Completed` with legal transitions; `Completed` terminal.
- Pure `schedule::next_delay` computes interval + cron next-fire (5-field accepted), errors on bad cron.
- Supervisor: shared `run_once`; daemon loop unchanged in behaviour; batch runs exactly N then
  `Completed`/`Failed`; scheduled fires repeatedly on interval (Skip overlap).
- Manager spawns 1 instance for scheduled/batch, N for daemon.
- `cargo test` green across the workspace; `cargo build --workspace` clean.

## Out of scope (later 2c sub-plans / follow-ups)
- Metrics sampling (2c.2), detectors (2c.3), IPC (2c.4), GUI (2c.5).
- Scheduled `Queue`/`KillPrevious` overlap (behaves as Skip for now — documented).
- Daemon-side scheduling persistence across restart beyond the existing reconcile.

## Self-review notes
- **Spec coverage:** implements spec §4 (run modes). Schedule/OverlapPolicy/Batch per the
  spec's enum sketch; corrected the spec's assumption that `Idle`/`Completed` already existed
  (Task 2 adds them).
- **Type consistency:** `RunMode::{Daemon,Scheduled,Batch}`, `Schedule::{Cron,IntervalSecs}`,
  `OverlapPolicy::{Skip,Queue,KillPrevious}`, `InstanceState::{Idle,Completed}`,
  `schedule::next_delay`, `InstanceSupervisor::{run_once,run_daemon,run_scheduled,run_batch,
  run_until_terminal}` used consistently. Manager concurrency match is exhaustive over all
  three variants.
- **Cron crate caveat:** Task 3 normalizes 5-field exprs to the seconds-leading 6-field form
  the `cron` crate expects; if the installed version differs, the task says to report the exact
  parse error and adjust `normalize_cron` while preserving the daily-3am test semantics.
