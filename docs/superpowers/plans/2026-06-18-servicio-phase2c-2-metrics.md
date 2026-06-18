# Servicio Phase 2c.2 — Metrics (sampling + storage + IPC) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Per-instance CPU%/memory sampling in the daemon (via `sysinfo`), rolling storage in SQLite, exposed over the socket as a `metrics` query method and a live `metric` event topic, with a `servicio metrics` CLI for headless verification.

**Architecture:** Add a `Metric` variant to `SupervisorEvent` (core). The daemon runs one sampler task @2s: read `Manager::status()` (pids), `sysinfo` refresh once, per live instance record `(worker,instance,ts,cpu,mem)` → insert into a new `metrics` SQLite table + emit a `Metric` event on the manager broadcast. A periodic prune bounds the table to ~1h. The `metrics{worker,since_secs}` IPC method queries the series; the subscribe forwarder maps `Metric` → a `metric` topic event.

**Tech Stack:** Rust, `sysinfo` crate, tokio, rusqlite, serde. Tests: `#[tokio::test]`, real `sh` processes, tempfile sockets.

**Builds on:** 2c.1 (merged). Spec: `docs/superpowers/specs/2026-06-18-servicio-phase2c-design.md` §5,§7.

---

## Task 1: SupervisorEvent::Metric (core, TDD)
**Files:** `crates/servicio-core/src/event.rs`

- [ ] **Step 1 — failing test.** Add to a `#[cfg(test)]` block in `event.rs` (create one if absent):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metric_event_roundtrips() {
        let e = SupervisorEvent::Metric { worker: "q".into(), instance: 0, ts: 1000, cpu: 3.5, mem: 1048576 };
        let back: SupervisorEvent = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(e, back);
    }
}
```
- [ ] **Step 2 — run, FAIL** (`cargo test -p servicio-core metric_event`).
- [ ] **Step 3 — implement.** Add a variant to `SupervisorEvent`:
```rust
    Metric { worker: String, instance: u32, ts: u64, cpu: f32, mem: u64 },
```
- [ ] **Step 4 — PASS** (`cargo test -p servicio-core`). Note: the daemon's subscribe forwarder (serve.rs) `match`es `SupervisorEvent` exhaustively — it will fail to compile until Task 5 handles `Metric`. To keep this task's crate green, only `cargo test -p servicio-core` here (not the workspace). Workspace compiles after Task 5.
- [ ] **Step 5 — commit:** `git add crates/servicio-core/src/event.rs && git commit -m "feat(core): Metric supervisor event variant"`

---

## Task 2: ipc metric types (TDD)
**Files:** `crates/servicio-ipc/src/types.rs`

- [ ] **Step 1 — failing test.** Add to `types.rs` tests:
```rust
    #[test]
    fn metric_series_roundtrips() {
        let s = MetricSeries { instance: 0, points: vec![MetricPoint { ts: 1, cpu: 2.0, mem: 3 }] };
        let back: MetricSeries = serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap();
        assert_eq!(s, back);
    }
```
- [ ] **Step 2 — run, FAIL.**
- [ ] **Step 3 — implement.** Add to `types.rs`:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricPoint { pub ts: u64, pub cpu: f32, pub mem: u64 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricSeries { pub instance: u32, pub points: Vec<MetricPoint> }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricEvent { pub worker: String, pub instance: u32, pub ts: u64, pub cpu: f32, pub mem: u64 }
```
(`PartialEq` on f32 is fine for tests; values are set literally.)
- [ ] **Step 4 — PASS** (`cargo test -p servicio-ipc`).
- [ ] **Step 5 — commit:** `git add crates/servicio-ipc/src/types.rs && git commit -m "feat(ipc): metric point/series/event types"`

---

## Task 3: Db metrics table (TDD)
**Files:** `crates/servicio-daemon/src/db.rs`

- [ ] **Step 1 — failing test.** Add to `db.rs` tests:
```rust
    #[test]
    fn metrics_insert_query_and_prune() {
        let db = Db::open_in_memory().unwrap();
        db.insert_metric("q", 0, 100, 1.5, 1000).unwrap();
        db.insert_metric("q", 0, 200, 2.5, 2000).unwrap();
        db.insert_metric("q", 1, 200, 0.5, 500).unwrap();
        let series = db.query_metrics("q", 0).unwrap(); // since ts >= 0
        // two instances → two series; instance 0 has 2 points
        let s0 = series.iter().find(|s| s.0 == 0).unwrap();
        assert_eq!(s0.1.len(), 2);
        // prune everything older than ts 150 → instance 0 keeps the ts=200 point only
        db.prune_metrics(150).unwrap();
        let after = db.query_metrics("q", 0).unwrap();
        let s0 = after.iter().find(|s| s.0 == 0).unwrap();
        assert_eq!(s0.1.len(), 1);
    }
```
- [ ] **Step 2 — run, FAIL.**
- [ ] **Step 3 — implement.** Extend `migrate()` with a metrics table (add to the `execute_batch`):
```sql
CREATE TABLE IF NOT EXISTS metrics (
    worker   TEXT NOT NULL,
    instance INTEGER NOT NULL,
    ts       INTEGER NOT NULL,
    cpu      REAL NOT NULL,
    mem      INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_metrics_worker_ts ON metrics(worker, ts);
```
Add methods to `impl Db`:
```rust
    pub fn insert_metric(&self, worker: &str, instance: u32, ts: u64, cpu: f32, mem: u64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO metrics (worker, instance, ts, cpu, mem) VALUES (?1,?2,?3,?4,?5)",
            rusqlite::params![worker, instance, ts as i64, cpu as f64, mem as i64],
        )?;
        Ok(())
    }

    /// Returns (instance, points) grouped, points = (ts,cpu,mem), for ts >= since.
    pub fn query_metrics(&self, worker: &str, since: u64) -> rusqlite::Result<Vec<(u32, Vec<(u64, f32, u64)>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT instance, ts, cpu, mem FROM metrics WHERE worker=?1 AND ts>=?2 ORDER BY instance, ts")?;
        let rows = stmt.query_map(rusqlite::params![worker, since as i64], |r| {
            Ok((r.get::<_, i64>(0)? as u32, r.get::<_, i64>(1)? as u64, r.get::<_, f64>(2)? as f32, r.get::<_, i64>(3)? as u64))
        })?;
        let mut out: Vec<(u32, Vec<(u64, f32, u64)>)> = Vec::new();
        for row in rows {
            let (inst, ts, cpu, mem) = row?;
            match out.last_mut() {
                Some((i, pts)) if *i == inst => pts.push((ts, cpu, mem)),
                _ => out.push((inst, vec![(ts, cpu, mem)])),
            }
        }
        Ok(out)
    }

    pub fn prune_metrics(&self, older_than_ts: u64) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM metrics WHERE ts < ?1", [older_than_ts as i64])?;
        Ok(())
    }
```
- [ ] **Step 4 — PASS** (`cargo test -p servicio-daemon db`).
- [ ] **Step 5 — commit:** `git add crates/servicio-daemon/src/db.rs && git commit -m "feat(daemon): metrics table insert/query/prune"`

---

## Task 4: Manager event sender accessor (core, TDD)
**Files:** `crates/servicio-core/src/manager.rs`

The sampler (in the daemon) needs to send `Metric` events on the manager's broadcast.

- [ ] **Step 1 — failing test.** Add to `manager.rs` tests:
```rust
    #[tokio::test]
    async fn events_sender_can_publish_to_subscribers() {
        use crate::event::SupervisorEvent;
        let dir = tempfile::tempdir().unwrap();
        let mgr = Manager::new(Arc::new(TokioProcess), dir.path().to_path_buf());
        let mut rx = mgr.subscribe();
        mgr.events_sender().send(SupervisorEvent::Metric { worker: "q".into(), instance: 0, ts: 1, cpu: 1.0, mem: 1 }).unwrap();
        assert!(matches!(rx.recv().await.unwrap(), SupervisorEvent::Metric { .. }));
    }
```
- [ ] **Step 2 — run, FAIL.**
- [ ] **Step 3 — implement.** Add to `impl Manager`:
```rust
    /// A clone of the broadcast sender so out-of-band producers (e.g. the metrics
    /// sampler) can publish events to all subscribers.
    pub fn events_sender(&self) -> tokio::sync::broadcast::Sender<crate::event::SupervisorEvent> {
        self.events.clone()
    }
```
- [ ] **Step 4 — PASS** (`cargo test -p servicio-core`).
- [ ] **Step 5 — commit:** `git add crates/servicio-core/src/manager.rs && git commit -m "feat(core): expose manager broadcast sender for out-of-band events"`

---

## Task 5: daemon sampler + metrics method + metric event (TDD integration)
**Files:** `crates/servicio-daemon/Cargo.toml`, `crates/servicio-daemon/src/sampler.rs` (new), `src/serve.rs`, `src/lib.rs`, `tests/serve_integration.rs`

- [ ] **Step 1 — add dep.** In `crates/servicio-daemon/Cargo.toml` `[dependencies]`: `sysinfo = "0.32"`.
- [ ] **Step 2 — sampler module.** Create `crates/servicio-daemon/src/sampler.rs`:
```rust
use crate::db::Db;
use servicio_core::event::SupervisorEvent;
use servicio_core::manager::Manager;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, System};
use tokio::sync::Mutex;

/// Sample every live instance's cpu/mem every 2s: store to DB + emit Metric events.
/// Prunes rows older than `retain_secs`. Runs until the process exits.
pub async fn run_sampler(
    manager: Arc<Mutex<Manager>>,
    db: Arc<Mutex<Db>>,
    retain_secs: u64,
) {
    let mut sys = System::new();
    let mut ticks: u64 = 0;
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

        let (snapshot, sender) = {
            let mgr = manager.lock().await;
            (mgr.status(), mgr.events_sender())
        };
        for w in snapshot {
            for inst in w.instances {
                if let Some(pid) = inst.pid {
                    if let Some(p) = sys.process(Pid::from_u32(pid)) {
                        let cpu = p.cpu_usage();
                        let mem = p.memory(); // bytes
                        {
                            let db = db.lock().await;
                            let _ = db.insert_metric(&w.name, inst.index, now, cpu, mem);
                        }
                        let _ = sender.send(SupervisorEvent::Metric {
                            worker: w.name.clone(), instance: inst.index, ts: now, cpu, mem,
                        });
                    }
                }
            }
        }
        ticks += 1;
        if ticks % 30 == 0 { // ~every 60s
            let cutoff = now.saturating_sub(retain_secs);
            let db = db.lock().await;
            let _ = db.prune_metrics(cutoff);
        }
    }
}
```
- [ ] **Step 3 — export + spawn.** In `lib.rs` add `pub mod sampler;`. In `serve.rs` `serve()`, after building the `Arc<Daemon>` (which holds `manager: Mutex<Manager>` and `db: Mutex<Db>`), spawn the sampler. The `Daemon` struct's `manager`/`db` are `Mutex` fields inside the `Arc<Daemon>` — pass clones of `Arc<Daemon>` field handles. Simplest: give the sampler `Arc<Daemon>` and have it lock `daemon.manager`/`daemon.db`. Adjust the sampler signature to take `Arc<Daemon>` instead, OR spawn:
```rust
    // after `let daemon = Arc::new(Daemon { ... });`
    {
        let d = Arc::clone(&daemon);
        tokio::spawn(async move {
            crate::sampler::run_sampler_for(d, 3600).await;
        });
    }
```
and add a thin wrapper in `sampler.rs`:
```rust
use crate::serve::Daemon;
pub async fn run_sampler_for(daemon: Arc<Daemon>, retain_secs: u64) {
    let mut sys = System::new();
    let mut ticks: u64 = 0;
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let (snapshot, sender) = {
            let mgr = daemon.manager.lock().await;
            (mgr.status(), mgr.events_sender())
        };
        for w in snapshot {
            for inst in w.instances {
                if let Some(pid) = inst.pid {
                    if let Some(p) = sys.process(Pid::from_u32(pid)) {
                        let cpu = p.cpu_usage();
                        let mem = p.memory();
                        { let db = daemon.db.lock().await; let _ = db.insert_metric(&w.name, inst.index, now, cpu, mem); }
                        let _ = sender.send(SupervisorEvent::Metric { worker: w.name.clone(), instance: inst.index, ts: now, cpu, mem });
                    }
                }
            }
        }
        ticks += 1;
        if ticks % 30 == 0 {
            let cutoff = now.saturating_sub(retain_secs);
            let db = daemon.db.lock().await; let _ = db.prune_metrics(cutoff);
        }
    }
}
```
(Make `Daemon`'s `manager`/`db` fields `pub` if not already; they are `pub` from Phase 2a.) You may delete the unused `run_sampler` from Step 2 and keep only `run_sampler_for`.
- [ ] **Step 4 — `metrics` dispatch + `Metric` forwarding.** In `serve.rs`:
  - In `dispatch`, add a `"metrics"` arm:
```rust
        "metrics" => {
            let name = params.get("worker").and_then(|n| n.as_str()).unwrap_or("").to_string();
            let since = params.get("since_secs").and_then(|n| n.as_u64()).unwrap_or(0);
            // since_secs is "last N seconds": convert to absolute floor.
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let floor = now.saturating_sub(since);
            let db = daemon.db.lock().await;
            match db.query_metrics(&name, if since == 0 { 0 } else { floor }) {
                Ok(rows) => {
                    let series: Vec<servicio_ipc::types::MetricSeries> = rows.into_iter().map(|(instance, pts)| {
                        servicio_ipc::types::MetricSeries {
                            instance,
                            points: pts.into_iter().map(|(ts,cpu,mem)| servicio_ipc::types::MetricPoint { ts, cpu, mem }).collect(),
                        }
                    }).collect();
                    match serde_json::to_value(series) { Ok(v) => Frame::ok(id, v), Err(e) => Frame::err(id, "internal", &e.to_string()) }
                }
                Err(e) => Frame::err(id, "db_error", &e.to_string()),
            }
        }
```
  - In `spawn_forwarder`'s event `match`, add a `Metric` arm that emits a `metric` topic event (respecting the `metric` topic filter):
```rust
                        SupervisorEvent::Metric { worker, instance, ts, cpu, mem } => {
                            if !topics.iter().any(|t| t == "metric") { continue; }
                            if let Some(f) = &worker_filter { if f != &worker { continue; } }
                            Frame::Event {
                                topic: "metric".into(),
                                payload: serde_json::to_value(servicio_ipc::types::MetricEvent { worker, instance, ts, cpu, mem }).unwrap(),
                            }
                        }
```
- [ ] **Step 5 — failing integration test.** Append to `tests/serve_integration.rs`:
```rust
#[tokio::test]
async fn metrics_method_returns_series_for_running_worker() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;
    let mut spec = sleeper("q");                      // sh -c "sleep 30", daemon ×1
    let _ = hello_then(&paths.socket(), vec![
        Frame::Request { id: 1, method: "add_worker".into(), params: json!({"spec": spec}) },
        Frame::Request { id: 2, method: "start_worker".into(), params: json!({"name":"q"}) },
    ]).await;
    // wait for >=2 sampler ticks (2s each)
    tokio::time::sleep(std::time::Duration::from_millis(5000)).await;
    let replies = hello_then(&paths.socket(), vec![
        Frame::Request { id: 1, method: "metrics".into(), params: json!({"worker":"q","since_secs":3600}) },
    ]).await;
    match &replies[1] {
        Frame::Response { id: 1, result: Some(v), .. } => {
            let arr = v.as_array().unwrap();
            assert!(!arr.is_empty(), "expected at least one instance series");
            let pts = arr[0]["points"].as_array().unwrap();
            assert!(pts.len() >= 1, "expected at least one sample point");
        }
        other => panic!("unexpected: {other:?}"),
    }
    h.shutdown().await;
    let _ = &mut spec;
}
```
(Remove the trailing `let _ = &mut spec;` if it warns; it's only to avoid an unused-mut note — adjust `let mut spec` to `let spec` if no mutation is needed.)
- [ ] **Step 6 — run.** `cargo test -p servicio-daemon --test serve_integration metrics_method` → PASS. Then full `cargo test` + `cargo build --workspace`. The 5s wait makes this test slow but deterministic.
- [ ] **Step 7 — commit:** `git add crates/servicio-daemon Cargo.lock && git commit -m "feat(daemon): metrics sampler + metrics method + metric event"`

---

## Task 6: servicio CLI `metrics` (TDD via e2e)
**Files:** `crates/servicio-cli/src/client.rs`, `src/main.rs`

- [ ] **Step 1 — client method.** Add to `client.rs` `impl Client`:
```rust
    pub async fn metrics(&mut self, worker: &str, since_secs: u64) -> Result<serde_json::Value> {
        self.request("metrics", json!({ "worker": worker, "since_secs": since_secs })).await
    }
```
- [ ] **Step 2 — CLI subcommand.** In `main.rs`, add a `Metrics { name: String }` variant to `Command` and a match arm:
```rust
        Command::Metrics { name } => {
            let v = client.metrics(&name, 900).await?;
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
```
- [ ] **Step 3 — verify.** `cargo build --workspace` clean; `cargo test` still green. (No new test required — covered by the daemon integration test; this is a thin client method + CLI wiring.)
- [ ] **Step 4 — commit:** `git add crates/servicio-cli && git commit -m "feat(cli): servicio metrics command"`

---

## Definition of Done (2c.2)
- `SupervisorEvent::Metric` + ipc `MetricPoint/Series/Event` roundtrip.
- Db `metrics` table: insert/query(grouped by instance)/prune.
- Daemon sampler @2s records cpu/mem per live instance to DB + emits `Metric`; prunes ~hourly.
- IPC `metrics{worker,since_secs}` returns per-instance series; `metric` topic streams live.
- `servicio metrics <name>` prints the series.
- `cargo test` + `cargo build --workspace` green.

## Out of scope
- GUI graphs (2c.5). cpu/mem in `list_workers` status (GUI reads metrics separately). Configurable sampling/retention.

## Self-review notes
- Spec §5/§7 covered. Types consistent: `SupervisorEvent::Metric` (core) ↔ ipc `MetricEvent`/`MetricSeries`/`MetricPoint`; `Db::{insert_metric,query_metrics,prune_metrics}`; `Manager::events_sender`; `sampler::run_sampler_for(Arc<Daemon>, retain)`; serve `metrics` dispatch + `metric` forward; `Client::metrics`.
- `sysinfo` 0.32 API: `System::new`, `refresh_processes(ProcessesToUpdate::All, true)`, `process(Pid)`, `.cpu_usage()`, `.memory()` (bytes). If the installed 0.32.x API differs (e.g. `refresh_processes` arity), the implementer adjusts to the actual signature and reports it.
