# Servicio Phase 2c — Run Modes, Metrics, Autodetect, Wizard, Notifications

**Date:** 2026-06-18
**Status:** Approved (design); implementation plans pending
**Builds on:** Phases 1, 2a, 2b (engine + daemon + socket API + CLI + minimal Tauri GUI), all merged.
**Parent design:** `docs/superpowers/specs/2026-06-18-servicio-design.md`

## 1. Summary

Phase 2c completes the original product vision's interactive core: it adds the **scheduled
and batch run modes** to the engine, **resource-metrics sampling + graphs**, a
**`servicio-detect`** crate with six framework detectors, the daemon **IPC** to expose
them, and the **GUI** that ties it together — a folder-scan autodetect step feeding a
4-step creation wizard, live metrics graphs on the worker detail, and native OS
notifications. It is one spec, built and shipped as five sequenced sub-plans (each its own
plan → build → merge cycle, each producing working software).

### Goals
- Engine: `Scheduled` (cron/interval) and `Batch` (run-N) run modes alongside `Daemon`.
- Engine: per-instance CPU/memory sampling, rolling storage, query + live events.
- `servicio-detect`: pluggable `Detector` trait + Laravel/Python/Node/Procfile/Crontab/Generic.
- Daemon IPC: `detect_workers`, `metrics` methods; `metric` event topic.
- GUI: detect → 4-step wizard (Command → Mode → Recovery → Review); Metrics tab with live
  graphs; native notifications on crash/crash-loop/recovery.

### Non-goals (2c)
- Remote/multi-machine control, TLS.
- OS-service install / run-on-boot; code signing / packaging (separate packaging phase).
- Plugin API for third-party detectors (the trait is internal for now).
- Log/metric export, historical dashboards beyond the rolling window.

## 2. Decisions (from brainstorming)

| Topic | Decision |
|---|---|
| Scope | Everything in one phase: run modes + metrics + autodetect + wizard + notifications. |
| Run modes | Add `Scheduled { cron/interval, overlap }` + `Batch { run_count, delay }` to the engine. |
| Detectors v1 | All six: Laravel, Python, Node, Procfile, Crontab, Generic. |
| Metrics source | `sysinfo` crate; single daemon sampler task @ 2s; rolling 1h in SQLite. |
| Detect behavior | Suggestions only; never auto-creates; each editable in the wizard. |
| Create path | Reuse `add_worker` (wizard sends a full `WorkerSpec`). |
| Notifications | Tauri `notification` plugin, fired by the frontend on state transitions. |
| GUI quality | Every component built with the `frontend-design` skill, matching the 2b aesthetic. |
| Delivery | One spec; five sequenced sub-plans. |

## 3. Build decomposition (five sub-plans)

1. **Engine run modes** — `Scheduled` + `Batch` variants, scheduler + batch supervisors,
   Manager integration, state transitions. (`servicio-core`)
2. **Engine metrics** — `sysinfo` sampler in the daemon, rolling `metrics` table, latest-in-
   status, prune. (`servicio-core` exposes pid; daemon owns the sampler + DB)
3. **`servicio-detect` crate** — `Detector` trait + six detectors + `detect_all` dedup. (pure)
4. **Daemon IPC** — `detect_workers` + `metrics` methods, `metric` event topic; CLI flags
   (`servicio detect <path>`, `servicio metrics <name>`).
5. **GUI** — detect→wizard flow, Metrics tab + graphs, notifications. (`apps/desktop`,
   frontend-design)

Each sub-plan is independently testable and merges to main before the next.

## 4. Engine run modes (sub-plan 1)

Extend `RunMode` (today only `Daemon { concurrency }`):

```rust
pub enum RunMode {
    Daemon    { concurrency: u32 },
    Scheduled { schedule: Schedule, overlap: OverlapPolicy },
    Batch     { run_count: u32, delay_secs: u64 },
}
pub enum Schedule { Cron(String), IntervalSecs(u64) }
pub enum OverlapPolicy { Skip, Queue, KillPrevious }
```

- **Cron parsing:** add a maintained cron crate (e.g. `croner` or `saffron`) to compute the
  next fire time from an expression; `IntervalSecs` is trivial.
- **Supervisors:** keep `InstanceSupervisor` for daemon. Add a `ScheduledSupervisor` (loops:
  sleep-until-next-fire → run one instance to completion → repeat; `OverlapPolicy` decides
  behaviour if a run is still active at the next tick) and a `BatchSupervisor` (run up to
  `run_count`, optional `delay_secs` between, then terminal). All expose the same
  `run_until_terminal(&self)` and event/state interface so `Manager` treats them uniformly
  (a small enum or boxed trait object per worker).
- **Lifecycle:** Scheduled uses `Idle ↔ Running`; Batch ends `Completed`/`Failed`. The
  `InstanceState` machine already has these states; add the needed legal transitions and
  emit them on the broadcast (reusing 2a's event wiring).
- **Restart policy** still applies within an individual scheduled/batch run.

## 5. Engine metrics (sub-plan 2)

- **Source:** `sysinfo` crate (cross-platform per-PID CPU% + RSS).
- **Sampler:** one daemon task ticks every 2s, does a single
  `System::refresh_processes`, then for each live instance PID (the manager already exposes
  per-instance pid) records `(worker, instance, ts, cpu, mem)`:
  - updates an in-memory "latest" map → surfaced in `list_workers` status + the UI,
  - appends a row to the SQLite `metrics` table,
  - emits a `metric` event on the broadcast.
- **Retention:** rolling window — keep the last 1 hour; a periodic prune deletes older rows.
- **Query:** `metrics { worker, since_secs }` returns per-instance series
  `[{ instance, points: [{ ts, cpu, mem }] }]`.
- **Cost:** one refresh per tick (not per instance); sampling is O(processes) and cheap.

## 6. `servicio-detect` crate (sub-plan 3)

New pure crate (reads the scanned folder only; no other IO).

```rust
pub struct SuggestionDraft {     // a proposed worker the user confirms/edits
    pub label: String,           // "Laravel queue worker"
    pub source: String,          // detector + matched file
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub run_mode: RunMode,        // suggested (daemon/scheduled/batch)
}
pub trait Detector {
    fn name(&self) -> &str;
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft>;
}
pub fn detect_all(root: &Path) -> Vec<SuggestionDraft>;  // all detectors + dedup by (command,args,working_dir)
```

| Detector | Looks for | Suggests |
|---|---|---|
| Laravel | `artisan` (+ `composer.json`) | `php artisan queue:work` (daemon ×2); `schedule:run` (scheduled `* * * * *`); `horizon`/`reverb` if in composer.json (daemon) |
| Python | venv (`bin/python`) / `requirements.txt` / top-level `*.py` | `python <script>` (daemon or batch) |
| Node | `package.json` scripts named like worker/queue; BullMQ dep | `npm run <script>` (daemon) |
| Procfile | each `name: command` line | one daemon worker per line |
| Crontab | a `crontab` file / `cron.d` entries | scheduled worker per line, cron expr → `Scheduled{Cron}` |
| Generic | always | one empty draft so the wizard can start from scratch |

Suggestions are never auto-created — they flow to the GUI, which lets the user
check/uncheck and edit each before calling `add_worker`. Pure + unit-testable with
`tempfile` fixture trees.

## 7. Daemon IPC (sub-plan 4)

Extends the 2a protocol (versioned; older clients unaffected).

| Method | Params | Returns |
|---|---|---|
| `detect_workers` | `{ path }` | `[ SuggestionDraft ]` (runs `servicio_detect::detect_all`) |
| `metrics` | `{ worker, since_secs }` | `[{ instance, points: [{ ts, cpu, mem }] }]` |

New event topic on the broadcast (clients `subscribe`): `metric` —
`{ worker, instance, ts, cpu, mem }` per sample.

`add_worker` is unchanged — it already accepts a full `WorkerSpec`, so scheduled/batch
specs and edited suggestions all flow through it. CLI gains `servicio detect <path>` and
`servicio metrics <name>` for headless testing.

## 8. GUI (sub-plan 5)

**Every component built with the `frontend-design` skill, matching the existing Phase-2b
aesthetic** (dark sidebar, copper `signal` accent, signal status palette, JetBrains-Mono
metrics/logs, control-room feel). No regression of the existing look.

- **Entry flow:** "+ New worker" opens **Detect** (step 1): a folder field + Browse (Tauri
  dialog) + Scan → calls `detect_workers` → renders suggestion rows (checkbox, label,
  command, run-mode chip, ✎ edit). "Start from scratch" skips to the blank wizard. "Add N
  selected → Review" proceeds.
- **Wizard (per worker):** Command → **Mode** → Recovery → Review. The Mode step has
  Daemon/Scheduled/Batch tabs that swap the options block (concurrency / cron+interval+overlap
  / run_count+delay). Review shows the final `WorkerSpec`; confirm calls `add_worker`.
- **Metrics tab** on worker detail: live CPU% + memory sparkline graphs (subscribe to
  `metric` events; seed from `metrics{since_secs}`), with a window selector (e.g. 15m/1h)
  and current-value tiles. Lightweight SVG/canvas charts (no heavy chart lib unless needed).
- **Notifications:** Tauri `notification` plugin; the frontend's existing event listener
  fires a native notification when an instance transitions to `crashed` or `failed`, or
  recovers (`crashed/backoff → running`). Permission requested once; a Settings toggle
  enables/disables. The in-app error toast (2b) stays.

## 9. Testing

- **Run modes:** cron next-fire calc + interval firing + overlap policies (real cheap
  processes); batch runs exactly N then `Completed`, failure → `Failed`. TDD.
- **Metrics:** sampler updates latest + appends rows; prune bounds the table; `metrics`
  query returns the series. Assert shape + monotonic timestamps, not exact cpu/mem numbers
  (environment-dependent).
- **`servicio-detect`:** each detector against `tempfile` fixture trees (`artisan`,
  `Procfile`, `crontab`, `package.json`, venv); `detect_all` dedup.
- **Daemon IPC:** `detect_workers` over the socket against a fixture dir; `metrics` returns a
  series; `metric` events stream end-to-end.
- **GUI:** Vitest — detect-results selection state, wizard mode-switching builds the correct
  spec, metrics graph data mapping, notification trigger logic (mocked plugin). Bridge
  integration tests vs a real daemon. frontend-design applied to every component.

## 10. Open questions / future
- Cron crate choice (`croner` vs `saffron`) — plan picks the maintained one with a clean API.
- Chart approach (hand-rolled SVG vs a tiny lib) — plan decides; prefer no heavy dep.
- Metrics retention/sampling are fixed defaults (2s / 1h) for v1; make configurable later.
- Carried from earlier phases: extract a tokio-free `servicio-types` leaf crate; constant-
  time token compare; event-pump reconnect; multi-toast queue.
