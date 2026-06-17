# Servicio Phase 1 — Core Engine + Daemon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a headless, fully-tested Rust supervisor that runs always-on (daemon-mode) workers, restarts them on crash with exponential backoff and a crash-loop guard, captures their logs to disk, persists definitions+state to SQLite, and self-reconciles after a restart — driven by a small test CLI.

**Architecture:** A Cargo workspace with two crates. `servicio-core` is a pure, async (Tokio) supervisor library with no UI and no service-install dependencies: domain types, a backoff calculator, an instance state machine, a process abstraction (trait + Unix impl), a log sink, a per-instance supervisor, and a multi-worker manager. `servicio-daemon` is a binary that adds SQLite persistence, reconcile-on-startup, and a `clap` CLI to add/list/start/stop/run workers for manual + integration testing. Scheduling, batch mode, IPC, and the GUI are explicitly out of this phase.

**Tech Stack:** Rust (edition 2021), Tokio (async runtime + process), serde + serde_json, rusqlite (bundled SQLite), clap (CLI), thiserror (errors), tracing (logging). Tests use Tokio's `#[tokio::test]` and `tempfile` for scratch dirs.

---

## File Structure

```
servicio/
  Cargo.toml                         # workspace manifest
  crates/
    servicio-core/
      Cargo.toml
      src/
        lib.rs                       # re-exports, crate docs
        error.rs                     # CoreError (thiserror)
        worker.rs                    # WorkerSpec, RunMode, RestartPolicy, RestartKind
        backoff.rs                   # Backoff: pure exponential-backoff + crash-loop calc
        state.rs                     # InstanceState enum + legal transitions
        process.rs                   # ProcessHandle trait, Spawned, ExitStatus, TokioProcess impl
        logsink.rs                   # LogSink: line-tagged capture to a rotating file
        supervisor.rs                # InstanceSupervisor: spawn→monitor→restart loop
        manager.rs                   # Manager: owns many workers, start/stop/status
    servicio-daemon/
      Cargo.toml
      src/
        main.rs                      # CLI entrypoint, wires db + manager
        db.rs                        # SQLite open/migrate, worker CRUD, state snapshot
        cli.rs                       # clap command definitions + handlers
  tests/                             # (per-crate, see each task)
```

Each file has one responsibility. `servicio-core` never imports rusqlite or clap — persistence and CLI live only in `servicio-daemon`, so the engine stays unit-testable in isolation.

---

## Task 0: Workspace + core crate skeleton

**Files:**
- Create: `Cargo.toml` (workspace)
- Create: `crates/servicio-core/Cargo.toml`
- Create: `crates/servicio-core/src/lib.rs`

- [ ] **Step 1: Create the workspace manifest**

Create `Cargo.toml`:

```toml
[workspace]
resolver = "2"
# servicio-daemon is added to members in Task 8 (it does not exist yet; cargo
# fails to load a workspace whose declared member dir is missing).
members = ["crates/servicio-core"]

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "MIT"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
tracing = "0.1"
rusqlite = { version = "0.31", features = ["bundled"] }
clap = { version = "4", features = ["derive"] }
tempfile = "3"
```

- [ ] **Step 2: Create the core crate manifest**

Create `crates/servicio-core/Cargo.toml`:

```toml
[package]
name = "servicio-core"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 3: Create a minimal lib.rs**

Create `crates/servicio-core/src/lib.rs`:

```rust
//! servicio-core: headless supervisor engine.
//!
//! Pure library — no UI, no SQLite, no service install. Spawns and monitors
//! worker processes, restarts them per policy, and captures their logs.

pub mod backoff;
pub mod error;
pub mod logsink;
pub mod manager;
pub mod process;
pub mod state;
pub mod supervisor;
pub mod worker;

pub use error::CoreError;
```

The build will fail until later tasks create the listed modules. That is expected; the next task starts filling them in.

- [ ] **Step 4: Create the error module so the crate compiles**

Create `crates/servicio-core/src/error.rs`:

```rust
use thiserror::Error;

/// All fallible operations in servicio-core return this error.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("working directory does not exist: {0}")]
    MissingWorkingDir(String),

    #[error("failed to spawn process: {0}")]
    Spawn(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid state transition: {from} -> {to}")]
    BadTransition { from: String, to: String },
}
```

- [ ] **Step 5: Stub the remaining modules so `cargo build` passes**

Create each of these as empty-but-valid files (later tasks replace their contents):

`crates/servicio-core/src/backoff.rs`:
```rust
// Replaced in Task 1.
```
`crates/servicio-core/src/worker.rs`:
```rust
// Replaced in Task 2.
```
`crates/servicio-core/src/state.rs`:
```rust
// Replaced in Task 3.
```
`crates/servicio-core/src/process.rs`:
```rust
// Replaced in Task 4.
```
`crates/servicio-core/src/logsink.rs`:
```rust
// Replaced in Task 5.
```
`crates/servicio-core/src/supervisor.rs`:
```rust
// Replaced in Task 6.
```
`crates/servicio-core/src/manager.rs`:
```rust
// Replaced in Task 7.
```

- [ ] **Step 6: Verify it builds**

Run: `cargo build -p servicio-core`
Expected: compiles with warnings about unused error variants. No errors.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/servicio-core
git commit -m "chore: scaffold cargo workspace and servicio-core skeleton"
```

---

## Task 1: Backoff calculator (pure, TDD)

**Files:**
- Modify: `crates/servicio-core/src/backoff.rs`

The backoff is pure arithmetic — no async, no IO — so it is the ideal first real unit.

- [ ] **Step 1: Write the failing tests**

Replace `crates/servicio-core/src/backoff.rs`:

```rust
use std::time::Duration;

#[cfg(test)]
mod tests {
    use super::*;

    fn b() -> Backoff {
        Backoff::new(Duration::from_secs(1), Duration::from_secs(60), 5, Duration::from_secs(30))
    }

    #[test]
    fn first_delay_is_base() {
        assert_eq!(b().delay_for_attempt(1), Duration::from_secs(1));
    }

    #[test]
    fn delay_doubles_each_attempt() {
        let bo = b();
        assert_eq!(bo.delay_for_attempt(2), Duration::from_secs(2));
        assert_eq!(bo.delay_for_attempt(3), Duration::from_secs(4));
        assert_eq!(bo.delay_for_attempt(4), Duration::from_secs(8));
    }

    #[test]
    fn delay_is_capped_at_max() {
        // attempt 10 would be 512s uncapped; cap is 60s.
        assert_eq!(b().delay_for_attempt(10), Duration::from_secs(60));
    }

    #[test]
    fn crash_loop_trips_after_max_retries() {
        assert!(!b().is_crash_loop(5)); // exactly at limit is still allowed
        assert!(b().is_crash_loop(6));  // one past the limit trips
    }

    #[test]
    fn uptime_beyond_reset_window_resets_counter() {
        let bo = b();
        assert!(bo.should_reset(Duration::from_secs(31)));
        assert!(!bo.should_reset(Duration::from_secs(29)));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p servicio-core backoff`
Expected: FAIL — `cannot find type Backoff in this scope`.

- [ ] **Step 3: Write the minimal implementation**

Add above the `#[cfg(test)]` block in `crates/servicio-core/src/backoff.rs`:

```rust
/// Exponential backoff + crash-loop detection. Pure, deterministic.
#[derive(Debug, Clone, Copy)]
pub struct Backoff {
    base: Duration,
    max: Duration,
    max_retries: u32,
    reset_window: Duration,
}

impl Backoff {
    pub fn new(base: Duration, max: Duration, max_retries: u32, reset_window: Duration) -> Self {
        Self { base, max, max_retries, reset_window }
    }

    /// Delay before retry `attempt` (1-based): base * 2^(attempt-1), capped at `max`.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }
        let factor = 2u64.saturating_pow(attempt - 1);
        let secs = self.base.as_secs().saturating_mul(factor);
        Duration::from_secs(secs).min(self.max)
    }

    /// True when the retry count has exceeded `max_retries`.
    pub fn is_crash_loop(&self, retries: u32) -> bool {
        retries > self.max_retries
    }

    /// True when an instance stayed up long enough to reset its retry counter.
    pub fn should_reset(&self, uptime: Duration) -> bool {
        uptime > self.reset_window
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p servicio-core backoff`
Expected: PASS — 5 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/servicio-core/src/backoff.rs
git commit -m "feat(core): exponential backoff + crash-loop calculator"
```

---

## Task 2: Worker domain types (serde, TDD)

**Files:**
- Modify: `crates/servicio-core/src/worker.rs`

- [ ] **Step 1: Write the failing tests**

Replace `crates/servicio-core/src/worker.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_spec_roundtrips_through_json() {
        let spec = WorkerSpec {
            name: "laravel-queue".into(),
            command: "php".into(),
            args: vec!["artisan".into(), "queue:work".into()],
            working_dir: PathBuf::from("/tmp"),
            env: BTreeMap::new(),
            run_mode: RunMode::Daemon { concurrency: 4 },
            restart: RestartPolicy::default(),
            autostart: true,
            enabled: true,
        };
        let json = serde_json::to_string(&spec).unwrap();
        let back: WorkerSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, back);
    }

    #[test]
    fn default_restart_policy_is_on_failure() {
        let p = RestartPolicy::default();
        assert_eq!(p.kind, RestartKind::OnFailure);
        assert_eq!(p.max_retries, 5);
    }

    #[test]
    fn concurrency_defaults_to_one_when_unset() {
        let mode: RunMode = serde_json::from_str(r#"{"type":"daemon"}"#).unwrap();
        assert_eq!(mode, RunMode::Daemon { concurrency: 1 });
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p servicio-core worker`
Expected: FAIL — `cannot find type WorkerSpec in this scope`.

- [ ] **Step 3: Write the minimal implementation**

Add above the `#[cfg(test)]` block in `crates/servicio-core/src/worker.rs`:

```rust
fn default_concurrency() -> u32 { 1 }

/// How a worker is run. Phase 1 supports Daemon only; later phases add
/// Scheduled and Batch as new variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunMode {
    Daemon {
        #[serde(default = "default_concurrency")]
        concurrency: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestartKind {
    Always,
    OnFailure,
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestartPolicy {
    pub kind: RestartKind,
    pub max_retries: u32,
    /// base/max/reset are seconds; mapped to Backoff by the supervisor.
    pub base_secs: u64,
    pub max_secs: u64,
    pub reset_window_secs: u64,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self { kind: RestartKind::OnFailure, max_retries: 5, base_secs: 1, max_secs: 60, reset_window_secs: 30 }
    }
}

/// A worker definition. The unit a user creates; the manager spawns instances from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerSpec {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub run_mode: RunMode,
    #[serde(default)]
    pub restart: RestartPolicy,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub enabled: bool,
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p servicio-core worker`
Expected: PASS — 3 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/servicio-core/src/worker.rs
git commit -m "feat(core): worker spec, run mode, and restart policy types"
```

---

## Task 3: Instance state machine (TDD)

**Files:**
- Modify: `crates/servicio-core/src/state.rs`

- [ ] **Step 1: Write the failing tests**

Replace `crates/servicio-core/src/state.rs`:

```rust
use crate::error::CoreError;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_path_running_to_crashed_to_backoff() {
        assert!(InstanceState::Starting.can_transition_to(InstanceState::Running));
        assert!(InstanceState::Running.can_transition_to(InstanceState::Crashed));
        assert!(InstanceState::Crashed.can_transition_to(InstanceState::Backoff));
        assert!(InstanceState::Backoff.can_transition_to(InstanceState::Starting));
    }

    #[test]
    fn illegal_transition_is_rejected() {
        assert!(!InstanceState::Stopped.can_transition_to(InstanceState::Running));
    }

    #[test]
    fn transition_returns_error_when_illegal() {
        let err = InstanceState::Stopped.transition_to(InstanceState::Running).unwrap_err();
        assert!(matches!(err, CoreError::BadTransition { .. }));
    }

    #[test]
    fn is_terminal_only_for_stopped_and_failed() {
        assert!(InstanceState::Stopped.is_terminal());
        assert!(InstanceState::Failed.is_terminal());
        assert!(!InstanceState::Running.is_terminal());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p servicio-core state`
Expected: FAIL — `cannot find type InstanceState in this scope`.

- [ ] **Step 3: Write the minimal implementation**

Add above the `#[cfg(test)]` block in `crates/servicio-core/src/state.rs`:

```rust
/// Lifecycle of a single running instance.
/// `Failed` = crash-loop tripped (gave up). `Stopped` = stopped by user/policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Crashed,
    Backoff,
    Failed,
}

impl InstanceState {
    pub fn is_terminal(self) -> bool {
        matches!(self, InstanceState::Stopped | InstanceState::Failed)
    }

    pub fn can_transition_to(self, to: InstanceState) -> bool {
        use InstanceState::*;
        matches!(
            (self, to),
            (Stopped, Starting)
                | (Starting, Running)
                | (Starting, Crashed)
                | (Running, Stopping)
                | (Running, Crashed)
                | (Stopping, Stopped)
                | (Crashed, Backoff)
                | (Crashed, Failed)
                | (Backoff, Starting)
                | (Backoff, Stopped)
        )
    }

    pub fn transition_to(self, to: InstanceState) -> Result<InstanceState, CoreError> {
        if self.can_transition_to(to) {
            Ok(to)
        } else {
            Err(CoreError::BadTransition { from: format!("{self:?}"), to: format!("{to:?}") })
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p servicio-core state`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/servicio-core/src/state.rs
git commit -m "feat(core): instance lifecycle state machine"
```

---

## Task 4: Process abstraction + Tokio impl (TDD)

**Files:**
- Modify: `crates/servicio-core/src/process.rs`

This isolates OS process control behind a trait so the supervisor is testable with fakes and portable across platforms later. Phase 1 ships a Tokio-backed Unix-friendly implementation.

- [ ] **Step 1: Write the failing tests**

Replace `crates/servicio-core/src/process.rs`:

```rust
use crate::error::CoreError;
use crate::worker::WorkerSpec;
use std::process::ExitStatus;
use tokio::io::AsyncRead;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::{RestartPolicy, RunMode};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use tokio::io::{AsyncBufReadExt, BufReader};

    fn spec(cmd: &str, args: &[&str]) -> WorkerSpec {
        WorkerSpec {
            name: "t".into(),
            command: cmd.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            working_dir: PathBuf::from("/"),
            env: BTreeMap::new(),
            run_mode: RunMode::Daemon { concurrency: 1 },
            restart: RestartPolicy::default(),
            autostart: false,
            enabled: true,
        }
    }

    #[tokio::test]
    async fn spawns_and_captures_stdout_then_exits_zero() {
        let mut spawned = TokioProcess.spawn(&spec("echo", &["hello"])).unwrap();
        let mut lines = BufReader::new(spawned.stdout.take().unwrap()).lines();
        let first = lines.next_line().await.unwrap().unwrap();
        assert_eq!(first, "hello");
        let status = spawned.wait().await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn nonzero_exit_is_reported_as_failure() {
        let mut spawned = TokioProcess.spawn(&spec("sh", &["-c", "exit 3"])).unwrap();
        let status = spawned.wait().await.unwrap();
        assert!(!status.success());
        assert_eq!(status.code(), Some(3));
    }

    #[tokio::test]
    async fn missing_working_dir_is_rejected_before_spawn() {
        let mut s = spec("echo", &["x"]);
        s.working_dir = PathBuf::from("/no/such/dir/servicio-xyz");
        let err = TokioProcess.spawn(&s).unwrap_err();
        assert!(matches!(err, CoreError::MissingWorkingDir(_)));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p servicio-core process`
Expected: FAIL — `cannot find type TokioProcess in this scope`.

- [ ] **Step 3: Write the minimal implementation**

Add above the `#[cfg(test)]` block in `crates/servicio-core/src/process.rs`:

```rust
use tokio::process::{Child, Command};

/// A live child process with its stdout/stderr pipes detached for the caller to read.
pub struct Spawned {
    child: Child,
    pub stdout: Option<Box<dyn AsyncRead + Unpin + Send>>,
    pub stderr: Option<Box<dyn AsyncRead + Unpin + Send>>,
}

// `Child` and the boxed `dyn AsyncRead` pipes are not `Debug`, but the tests call
// `.unwrap_err()` (which requires the `Ok` type to be `Debug`). Provide a manual impl.
impl std::fmt::Debug for Spawned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spawned").field("pid", &self.child.id()).finish_non_exhaustive()
    }
}

impl Spawned {
    /// The OS process id, if still available.
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Wait for the process to exit.
    pub async fn wait(&mut self) -> Result<ExitStatus, CoreError> {
        Ok(self.child.wait().await?)
    }

    /// Ask the process to stop (graceful). Unix: SIGTERM via kill(); Windows impl added later.
    pub async fn terminate(&mut self) -> Result<(), CoreError> {
        // start_kill sends SIGKILL on Unix today; a SIGTERM-then-SIGKILL refinement
        // lands with the platform-signals work in a later phase.
        self.child.start_kill()?;
        Ok(())
    }
}

/// Abstraction over "how do I start a process from a spec". Lets the supervisor be
/// tested against fakes and swapped per-platform later.
pub trait ProcessSpawner: Send + Sync + 'static {
    fn spawn(&self, spec: &WorkerSpec) -> Result<Spawned, CoreError>;
}

/// Real implementation backed by tokio::process.
pub struct TokioProcess;

impl ProcessSpawner for TokioProcess {
    fn spawn(&self, spec: &WorkerSpec) -> Result<Spawned, CoreError> {
        if !spec.working_dir.exists() {
            return Err(CoreError::MissingWorkingDir(spec.working_dir.display().to_string()));
        }
        let mut cmd = Command::new(&spec.command);
        cmd.args(&spec.args)
            .current_dir(&spec.working_dir)
            .env_clear()
            .envs(&spec.env)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| CoreError::Spawn(e.to_string()))?;
        let stdout = child.stdout.take().map(|s| Box::new(s) as Box<dyn AsyncRead + Unpin + Send>);
        let stderr = child.stderr.take().map(|s| Box::new(s) as Box<dyn AsyncRead + Unpin + Send>);
        Ok(Spawned { child, stdout, stderr })
    }
}
```

> Note: `env_clear()` then `envs()` gives the clean-env-plus-user-vars behaviour from the spec. PATH-sensitive commands (e.g. bare `php`) are resolved by absolute path or by adding PATH to `env` — the test CLI in Task 9 passes absolute/`sh -c` commands.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p servicio-core process`
Expected: PASS — 3 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/servicio-core/src/process.rs
git commit -m "feat(core): process spawner trait + tokio implementation"
```

---

## Task 5: Log sink with rotation (TDD)

**Files:**
- Modify: `crates/servicio-core/src/logsink.rs`

- [ ] **Step 1: Write the failing tests**

Replace `crates/servicio-core/src/logsink.rs`:

```rust
use crate::error::CoreError;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn writes_tagged_lines_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("w.log");
        let mut sink = LogSink::new(&path, 1_000_000, 3).unwrap();
        sink.write_line(2, "stdout", "processing job").unwrap();

        let mut contents = String::new();
        File::open(&path).unwrap().read_to_string(&mut contents).unwrap();
        assert!(contents.contains("[#2]"));
        assert!(contents.contains("stdout"));
        assert!(contents.contains("processing job"));
    }

    #[test]
    fn rotates_when_size_cap_exceeded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("w.log");
        // tiny cap forces rotation after the first line.
        let mut sink = LogSink::new(&path, 10, 3).unwrap();
        sink.write_line(1, "stdout", "first line is long enough").unwrap();
        sink.write_line(1, "stdout", "second").unwrap();

        // rotated file w.log.1 must now exist.
        assert!(path.with_extension("log.1").exists());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p servicio-core logsink`
Expected: FAIL — `cannot find type LogSink in this scope`.

- [ ] **Step 3: Write the minimal implementation**

Add above the `#[cfg(test)]` block in `crates/servicio-core/src/logsink.rs`:

```rust
/// Appends tagged log lines to a file, rotating by size. Synchronous and simple:
/// the supervisor calls it from a blocking-friendly context per line.
pub struct LogSink {
    path: PathBuf,
    file: File,
    written: u64,
    max_bytes: u64,
    max_files: u32,
}

impl LogSink {
    pub fn new(path: &Path, max_bytes: u64, max_files: u32) -> Result<Self, CoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let written = file.metadata()?.len();
        Ok(Self { path: path.to_path_buf(), file, written, max_bytes, max_files })
    }

    /// Write one line tagged with instance index + stream name + a timestamp marker.
    pub fn write_line(&mut self, instance: u32, stream: &str, line: &str) -> Result<(), CoreError> {
        let record = format!("[#{instance}] [{stream}] {line}\n");
        if self.written + record.len() as u64 > self.max_bytes {
            self.rotate()?;
        }
        self.file.write_all(record.as_bytes())?;
        self.written += record.len() as u64;
        Ok(())
    }

    /// Shift w.log -> w.log.1 -> w.log.2 ... dropping anything past max_files.
    fn rotate(&mut self) -> Result<(), CoreError> {
        for i in (1..self.max_files).rev() {
            let from = self.indexed(i);
            let to = self.indexed(i + 1);
            if from.exists() {
                std::fs::rename(&from, &to)?;
            }
        }
        std::fs::rename(&self.path, &self.indexed(1))?;
        self.file = OpenOptions::new().create(true).append(true).open(&self.path)?;
        self.written = 0;
        Ok(())
    }

    fn indexed(&self, i: u32) -> PathBuf {
        self.path.with_extension(format!("log.{i}"))
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p servicio-core logsink`
Expected: PASS — 2 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/servicio-core/src/logsink.rs
git commit -m "feat(core): size-rotating tagged log sink"
```

---

## Task 6: Instance supervisor (spawn → monitor → restart, TDD)

**Files:**
- Modify: `crates/servicio-core/src/supervisor.rs`

This is the heart of "always running." It runs one instance, captures its logs, and on exit decides whether to restart (with backoff) or give up (crash-loop → Failed).

- [ ] **Step 1: Write the failing tests**

Replace `crates/servicio-core/src/supervisor.rs`:

```rust
use crate::backoff::Backoff;
use crate::logsink::LogSink;
use crate::process::ProcessSpawner;
use crate::state::InstanceState;
use crate::worker::{RestartKind, WorkerSpec};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::watch;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::TokioProcess;
    use crate::worker::{RestartPolicy, RunMode};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn spec(cmd: &str, args: &[&str], restart: RestartPolicy) -> WorkerSpec {
        WorkerSpec {
            name: "t".into(),
            command: cmd.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            working_dir: PathBuf::from("/"),
            env: BTreeMap::new(),
            run_mode: RunMode::Daemon { concurrency: 1 },
            restart,
            autostart: false,
            enabled: true,
        }
    }

    #[tokio::test]
    async fn never_policy_runs_once_then_stops() {
        let dir = tempfile::tempdir().unwrap();
        let policy = RestartPolicy { kind: RestartKind::Never, ..Default::default() };
        let sup = InstanceSupervisor::new(
            1,
            spec("sh", &["-c", "exit 0"], policy),
            Arc::new(TokioProcess),
            dir.path().join("t.log"),
        );
        let mut state_rx = sup.subscribe();
        sup.run_until_terminal().await;
        assert_eq!(*state_rx.borrow_and_update(), InstanceState::Stopped);
        assert_eq!(sup.restart_count(), 0);
    }

    #[tokio::test]
    async fn on_failure_policy_restarts_until_crash_loop_then_fails() {
        let dir = tempfile::tempdir().unwrap();
        // base 0s so the test does not actually sleep; max_retries 2 → 3rd failure fails it.
        let policy = RestartPolicy {
            kind: RestartKind::OnFailure,
            max_retries: 2,
            base_secs: 0,
            max_secs: 0,
            reset_window_secs: 30,
        };
        let sup = InstanceSupervisor::new(
            1,
            spec("sh", &["-c", "exit 1"], policy),
            Arc::new(TokioProcess),
            dir.path().join("t.log"),
        );
        let mut state_rx = sup.subscribe();
        sup.run_until_terminal().await;
        assert_eq!(*state_rx.borrow_and_update(), InstanceState::Failed);
        // 1 initial run + 2 retries = 3 spawns; restart_count counts the retries.
        assert_eq!(sup.restart_count(), 2);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p servicio-core supervisor`
Expected: FAIL — `cannot find type InstanceSupervisor in this scope`.

- [ ] **Step 3: Write the minimal implementation**

Add above the `#[cfg(test)]` block in `crates/servicio-core/src/supervisor.rs`:

```rust
use std::path::PathBuf;
use std::time::Instant;

/// Supervises a single instance: spawn, pump logs, decide restart vs give-up.
pub struct InstanceSupervisor {
    index: u32,
    spec: WorkerSpec,
    spawner: Arc<dyn ProcessSpawner>,
    log_path: PathBuf,
    restarts: AtomicU32,
    state_tx: watch::Sender<InstanceState>,
    state_rx: watch::Receiver<InstanceState>,
}

impl InstanceSupervisor {
    pub fn new(
        index: u32,
        spec: WorkerSpec,
        spawner: Arc<dyn ProcessSpawner>,
        log_path: PathBuf,
    ) -> Self {
        let (state_tx, state_rx) = watch::channel(InstanceState::Stopped);
        Self { index, spec, spawner, log_path, restarts: AtomicU32::new(0), state_tx, state_rx }
    }

    pub fn subscribe(&self) -> watch::Receiver<InstanceState> {
        self.state_rx.clone()
    }

    pub fn restart_count(&self) -> u32 {
        self.restarts.load(Ordering::SeqCst)
    }

    fn set_state(&self, s: InstanceState) {
        let _ = self.state_tx.send(s);
    }

    fn backoff(&self) -> Backoff {
        let r = &self.spec.restart;
        Backoff::new(
            Duration::from_secs(r.base_secs),
            Duration::from_secs(r.max_secs),
            r.max_retries,
            Duration::from_secs(r.reset_window_secs),
        )
    }

    /// Run the spawn/monitor/restart loop until the instance reaches a terminal state.
    pub async fn run_until_terminal(&self) {
        let backoff = self.backoff();
        let mut retries: u32 = 0;
        let mut sink = LogSink::new(&self.log_path, 10 * 1024 * 1024, 5)
            .expect("log sink should open");

        loop {
            self.set_state(InstanceState::Starting);
            let started = Instant::now();

            let mut spawned = match self.spawner.spawn(&self.spec) {
                Ok(s) => s,
                Err(_) => {
                    // Treat spawn failure like a crash for restart accounting.
                    if !self.should_retry(&backoff, retries) {
                        self.set_state(InstanceState::Failed);
                        return;
                    }
                    retries += 1;
                    self.restarts.store(retries, Ordering::SeqCst);
                    self.sleep_backoff(&backoff, retries).await;
                    continue;
                }
            };
            self.set_state(InstanceState::Running);

            // Pump stdout lines into the sink concurrently with waiting for exit.
            if let Some(out) = spawned.stdout.take() {
                let mut lines = BufReader::new(out).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = sink.write_line(self.index, "stdout", &line);
                }
            }

            let status = spawned.wait().await;
            let success = status.map(|s| s.success()).unwrap_or(false);
            let uptime = started.elapsed();

            // Reset retry counter if the instance was stable long enough.
            if backoff.should_reset(uptime) {
                retries = 0;
                self.restarts.store(0, Ordering::SeqCst);
            }

            if !self.wants_restart(success) {
                self.set_state(InstanceState::Stopped);
                return;
            }

            self.set_state(InstanceState::Crashed);
            if !self.should_retry(&backoff, retries) {
                self.set_state(InstanceState::Failed);
                return;
            }
            retries += 1;
            self.restarts.store(retries, Ordering::SeqCst);
            self.set_state(InstanceState::Backoff);
            self.sleep_backoff(&backoff, retries).await;
        }
    }

    /// Does the restart policy want another run after this exit?
    fn wants_restart(&self, success: bool) -> bool {
        match self.spec.restart.kind {
            RestartKind::Always => true,
            RestartKind::OnFailure => !success,
            RestartKind::Never => false,
        }
    }

    /// Are we still under the crash-loop limit?
    fn should_retry(&self, backoff: &Backoff, retries: u32) -> bool {
        !backoff.is_crash_loop(retries + 1)
    }

    async fn sleep_backoff(&self, backoff: &Backoff, attempt: u32) {
        let d = backoff.delay_for_attempt(attempt);
        if !d.is_zero() {
            tokio::time::sleep(d).await;
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p servicio-core supervisor`
Expected: PASS — 2 tests.

- [ ] **Step 5: Run the whole core suite**

Run: `cargo test -p servicio-core`
Expected: PASS — all tests from Tasks 1–6.

- [ ] **Step 6: Commit**

```bash
git add crates/servicio-core/src/supervisor.rs
git commit -m "feat(core): instance supervisor with restart, backoff, crash-loop guard"
```

---

## Task 7: Manager — many workers (TDD)

**Files:**
- Modify: `crates/servicio-core/src/manager.rs`

The manager owns a set of workers, starts `concurrency` instances each, tracks them, and stops them.

- [ ] **Step 1: Write the failing tests**

Replace `crates/servicio-core/src/manager.rs`:

```rust
use crate::process::ProcessSpawner;
use crate::supervisor::InstanceSupervisor;
use crate::worker::{RunMode, WorkerSpec};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinHandle;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::TokioProcess;
    use crate::worker::{RestartKind, RestartPolicy, RunMode};
    use std::collections::BTreeMap;

    fn long_running(name: &str) -> WorkerSpec {
        WorkerSpec {
            name: name.into(),
            command: "sh".into(),
            args: vec!["-c".into(), "sleep 30".into()],
            working_dir: PathBuf::from("/"),
            env: BTreeMap::new(),
            run_mode: RunMode::Daemon { concurrency: 2 },
            restart: RestartPolicy { kind: RestartKind::Always, ..Default::default() },
            autostart: true,
            enabled: true,
        }
    }

    #[tokio::test]
    async fn start_worker_spawns_concurrency_instances() {
        let dir = tempfile::tempdir().unwrap();
        let mut mgr = Manager::new(Arc::new(TokioProcess), dir.path().to_path_buf());
        mgr.start_worker(long_running("q")).await;
        assert_eq!(mgr.instance_count("q"), 2);
        mgr.stop_all().await;
    }

    #[tokio::test]
    async fn stop_all_removes_instances() {
        let dir = tempfile::tempdir().unwrap();
        let mut mgr = Manager::new(Arc::new(TokioProcess), dir.path().to_path_buf());
        mgr.start_worker(long_running("q")).await;
        mgr.stop_all().await;
        assert_eq!(mgr.instance_count("q"), 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p servicio-core manager`
Expected: FAIL — `cannot find type Manager in this scope`.

- [ ] **Step 3: Write the minimal implementation**

Add above the `#[cfg(test)]` block in `crates/servicio-core/src/manager.rs`:

```rust
struct RunningInstance {
    handle: JoinHandle<()>,
}

/// Owns all workers and their running instances. One Manager per daemon.
pub struct Manager {
    spawner: Arc<dyn ProcessSpawner>,
    log_dir: PathBuf,
    instances: HashMap<String, Vec<RunningInstance>>,
}

impl Manager {
    pub fn new(spawner: Arc<dyn ProcessSpawner>, log_dir: PathBuf) -> Self {
        Self { spawner, log_dir, instances: HashMap::new() }
    }

    /// Start every instance for a worker per its concurrency, each in its own task.
    pub async fn start_worker(&mut self, spec: WorkerSpec) {
        let concurrency = match spec.run_mode {
            RunMode::Daemon { concurrency } => concurrency.max(1),
        };
        let mut started = Vec::new();
        for i in 0..concurrency {
            let log_path = self.log_dir.join(format!("{}-{}.log", spec.name, i));
            let sup = InstanceSupervisor::new(i, spec.clone(), Arc::clone(&self.spawner), log_path);
            let handle = tokio::spawn(async move { sup.run_until_terminal().await });
            started.push(RunningInstance { handle });
        }
        self.instances.insert(spec.name.clone(), started);
    }

    pub fn instance_count(&self, worker: &str) -> usize {
        self.instances.get(worker).map(|v| v.len()).unwrap_or(0)
    }

    /// Abort all running instance tasks (kill_on_drop terminates the children).
    pub async fn stop_all(&mut self) {
        for (_, list) in self.instances.drain() {
            for inst in list {
                inst.handle.abort();
            }
        }
    }
}
```

> Note: aborting the task drops the `Spawned` child, and `kill_on_drop(true)` (Task 4) terminates the OS process. A graceful SIGTERM-then-SIGKILL stop with per-instance handles is refined alongside the IPC work in Phase 2.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p servicio-core manager`
Expected: PASS — 2 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/servicio-core/src/manager.rs
git commit -m "feat(core): manager owning many workers and their instances"
```

---

## Task 8: Daemon crate — SQLite persistence (TDD)

**Files:**
- Create: `crates/servicio-daemon/Cargo.toml`
- Create: `crates/servicio-daemon/src/db.rs`
- Create: `crates/servicio-daemon/src/main.rs` (temporary minimal entry)

- [ ] **Step 0: Add the daemon to the workspace members**

Edit the root `Cargo.toml` `members` array to include the daemon now that it exists:

```toml
members = ["crates/servicio-core", "crates/servicio-daemon"]
```

- [ ] **Step 1: Create the daemon crate manifest**

Create `crates/servicio-daemon/Cargo.toml`:

```toml
[package]
name = "servicio-daemon"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
servicio-core = { path = "../servicio-core" }
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
rusqlite.workspace = true
clap.workspace = true
thiserror.workspace = true
tracing.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Create a temporary main so the crate compiles**

Create `crates/servicio-daemon/src/main.rs`:

```rust
mod db;

fn main() {
    println!("servicio-daemon");
}
```

- [ ] **Step 3: Write the failing tests**

Create `crates/servicio-daemon/src/db.rs`:

```rust
use rusqlite::Connection;
use servicio_core::worker::WorkerSpec;

#[cfg(test)]
mod tests {
    use super::*;
    use servicio_core::worker::{RestartPolicy, RunMode};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn spec(name: &str) -> WorkerSpec {
        WorkerSpec {
            name: name.into(),
            command: "sh".into(),
            args: vec!["-c".into(), "sleep 1".into()],
            working_dir: PathBuf::from("/"),
            env: BTreeMap::new(),
            run_mode: RunMode::Daemon { concurrency: 2 },
            restart: RestartPolicy::default(),
            autostart: true,
            enabled: true,
        }
    }

    #[test]
    fn migrate_then_upsert_and_list_roundtrips() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_worker(&spec("queue")).unwrap();
        let all = db.list_workers().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "queue");
        assert_eq!(all[0].run_mode, RunMode::Daemon { concurrency: 2 });
    }

    #[test]
    fn upsert_same_name_replaces_not_duplicates() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_worker(&spec("queue")).unwrap();
        let mut changed = spec("queue");
        changed.autostart = false;
        db.upsert_worker(&changed).unwrap();
        let all = db.list_workers().unwrap();
        assert_eq!(all.len(), 1);
        assert!(!all[0].autostart);
    }

    #[test]
    fn autostart_filter_returns_only_autostart_enabled() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_worker(&spec("yes")).unwrap();
        let mut no = spec("no");
        no.autostart = false;
        db.upsert_worker(&no).unwrap();
        let names: Vec<_> = db.autostart_workers().unwrap().into_iter().map(|w| w.name).collect();
        assert_eq!(names, vec!["yes".to_string()]);
    }
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test -p servicio-daemon db`
Expected: FAIL — `cannot find type Db in this scope`.

- [ ] **Step 5: Write the minimal implementation**

Add above the `#[cfg(test)]` block in `crates/servicio-daemon/src/db.rs`:

```rust
use std::path::Path;

/// SQLite persistence for worker definitions. Source of truth for the daemon.
pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS workers (
                name      TEXT PRIMARY KEY,
                spec_json TEXT NOT NULL,
                autostart INTEGER NOT NULL,
                enabled   INTEGER NOT NULL
            );",
        )
    }

    /// Insert or replace a worker by name. Full spec stored as JSON; a couple of
    /// columns are duplicated for cheap filtering.
    pub fn upsert_worker(&self, spec: &WorkerSpec) -> rusqlite::Result<()> {
        let json = serde_json::to_string(spec).expect("spec serializes");
        self.conn.execute(
            "INSERT INTO workers (name, spec_json, autostart, enabled)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(name) DO UPDATE SET
                spec_json = excluded.spec_json,
                autostart = excluded.autostart,
                enabled   = excluded.enabled",
            rusqlite::params![spec.name, json, spec.autostart as i64, spec.enabled as i64],
        )?;
        Ok(())
    }

    pub fn list_workers(&self) -> rusqlite::Result<Vec<WorkerSpec>> {
        self.query("SELECT spec_json FROM workers ORDER BY name")
    }

    /// Workers that should be (re)started automatically by the daemon.
    pub fn autostart_workers(&self) -> rusqlite::Result<Vec<WorkerSpec>> {
        self.query("SELECT spec_json FROM workers WHERE autostart = 1 AND enabled = 1 ORDER BY name")
    }

    fn query(&self, sql: &str) -> rusqlite::Result<Vec<WorkerSpec>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            let json: String = row.get(0)?;
            Ok(serde_json::from_str::<WorkerSpec>(&json).expect("stored spec parses"))
        })?;
        rows.collect()
    }
}
```

> Note: `WorkerSpec` and its fields must be `pub` in `servicio-core`. They are, from Task 2. If `cargo test` reports the module is private, add `pub mod worker;` — already present from Task 0's lib.rs.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p servicio-daemon db`
Expected: PASS — 3 tests.

- [ ] **Step 7: Commit**

```bash
git add crates/servicio-daemon
git commit -m "feat(daemon): sqlite persistence for worker definitions"
```

---

## Task 9: Daemon CLI + reconcile (TDD via integration test)

**Files:**
- Create: `crates/servicio-daemon/src/cli.rs`
- Modify: `crates/servicio-daemon/src/main.rs`
- Create: `crates/servicio-daemon/tests/cli_integration.rs`

This wires everything: a CLI to add/list workers, a `run` command that loads autostart workers and supervises them, proving end-to-end persistence + supervision.

- [ ] **Step 1: Write the failing integration test**

Create `crates/servicio-daemon/tests/cli_integration.rs`:

```rust
// End-to-end: add a worker via the library API the CLI uses, confirm it persists,
// and confirm reconcile picks up exactly the autostart workers.
use servicio_daemon_lib::{add_worker, reconcile_specs};
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn add_then_reconcile_loads_autostart_worker() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("servicio.db");

    add_worker(
        &db_path,
        "queue",
        "sh",
        &["-c".into(), "sleep 1".into()],
        &PathBuf::from("/"),
        2,
        true,
    )
    .unwrap();

    let specs = reconcile_specs(&db_path).unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].name, "queue");
}
```

> The CLI binary stays a thin shell over a small library module so it can be tested without spawning a subprocess. We expose that module as `servicio_daemon_lib`.

- [ ] **Step 2: Expose a lib target for the daemon**

Modify `crates/servicio-daemon/Cargo.toml` — add a `[lib]` section after `[package]`:

```toml
[lib]
name = "servicio_daemon_lib"
path = "src/lib.rs"

[[bin]]
name = "servicio-daemon"
path = "src/main.rs"
```

- [ ] **Step 3: Create the library entry that the test and binary share**

Create `crates/servicio-daemon/src/lib.rs`:

```rust
pub mod cli;
pub mod db;

use db::Db;
use servicio_core::worker::{RestartPolicy, RunMode, WorkerSpec};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Add (or replace) a worker definition in the database.
#[allow(clippy::too_many_arguments)]
pub fn add_worker(
    db_path: &Path,
    name: &str,
    command: &str,
    args: &[String],
    working_dir: &PathBuf,
    concurrency: u32,
    autostart: bool,
) -> rusqlite::Result<()> {
    let spec = WorkerSpec {
        name: name.to_string(),
        command: command.to_string(),
        args: args.to_vec(),
        working_dir: working_dir.clone(),
        env: BTreeMap::new(),
        run_mode: RunMode::Daemon { concurrency },
        restart: RestartPolicy::default(),
        autostart,
        enabled: true,
    };
    let db = Db::open(db_path)?;
    db.upsert_worker(&spec)
}

/// The reconcile step the daemon runs on startup: which workers should be running?
pub fn reconcile_specs(db_path: &Path) -> rusqlite::Result<Vec<WorkerSpec>> {
    let db = Db::open(db_path)?;
    db.autostart_workers()
}
```

- [ ] **Step 4: Run the integration test to verify it fails**

Run: `cargo test -p servicio-daemon --test cli_integration`
Expected: FAIL — unresolved import `servicio_daemon_lib` or missing `cli` module.

- [ ] **Step 5: Create the CLI module**

Create `crates/servicio-daemon/src/cli.rs`:

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "servicio-daemon", about = "Servicio supervisor (phase 1 test CLI)")]
pub struct Cli {
    /// Path to the SQLite database.
    #[arg(long, default_value = "servicio.db")]
    pub db: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Add or replace a worker definition.
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        command: String,
        #[arg(long, value_delimiter = ' ')]
        args: Vec<String>,
        #[arg(long, default_value = ".")]
        working_dir: PathBuf,
        #[arg(long, default_value_t = 1)]
        concurrency: u32,
        #[arg(long, default_value_t = true)]
        autostart: bool,
    },
    /// List stored workers.
    List,
    /// Load autostart workers and supervise them until Ctrl-C.
    Run,
}
```

- [ ] **Step 6: Replace main.rs to drive the CLI**

Replace `crates/servicio-daemon/src/main.rs`:

```rust
use clap::Parser;
use servicio_core::manager::Manager;
use servicio_core::process::TokioProcess;
use servicio_daemon_lib::cli::{Cli, Command};
use servicio_daemon_lib::{add_worker, db::Db, reconcile_specs};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Add { name, command, args, working_dir, concurrency, autostart } => {
            add_worker(&cli.db, &name, &command, &args, &working_dir, concurrency, autostart)?;
            println!("added worker '{name}'");
        }
        Command::List => {
            let db = Db::open(&cli.db)?;
            for w in db.list_workers()? {
                println!("{}  cmd={} {:?}  mode={:?}  autostart={}", w.name, w.command, w.args, w.run_mode, w.autostart);
            }
        }
        Command::Run => {
            let specs = reconcile_specs(&cli.db)?;
            let log_dir = std::env::temp_dir().join("servicio-logs");
            let mut mgr = Manager::new(Arc::new(TokioProcess), log_dir);
            for spec in specs {
                println!("starting '{}'", spec.name);
                mgr.start_worker(spec).await;
            }
            println!("supervising; press Ctrl-C to stop");
            tokio::signal::ctrl_c().await?;
            mgr.stop_all().await;
            println!("stopped");
        }
    }
    Ok(())
}
```

- [ ] **Step 7: Run the integration test to verify it passes**

Run: `cargo test -p servicio-daemon --test cli_integration`
Expected: PASS — 1 test.

- [ ] **Step 8: Manually smoke-test the binary**

Run:
```bash
cargo run -p servicio-daemon -- --db /tmp/servicio-smoke.db add --name hb --command sh --args "-c 'while true; do echo tick; sleep 1; done'" --concurrency 1
cargo run -p servicio-daemon -- --db /tmp/servicio-smoke.db list
```
Expected: `added worker 'hb'`, then a list line showing `hb`. (A full `run` test that supervises and Ctrl-C is manual.)

- [ ] **Step 9: Run the whole workspace test suite**

Run: `cargo test`
Expected: PASS — every test from Tasks 1–9.

- [ ] **Step 10: Commit**

```bash
git add crates/servicio-daemon
git commit -m "feat(daemon): test CLI with add/list/run + reconcile and integration test"
```

---

## Task 10: Integration test — real crash + restart end-to-end

**Files:**
- Create: `crates/servicio-core/tests/restart_integration.rs`

Proves the headline guarantee: a worker that crashes is actually restarted, observably, with a real process.

- [ ] **Step 1: Write the failing integration test**

Create `crates/servicio-core/tests/restart_integration.rs`:

```rust
// A worker that writes a marker file then exits non-zero must be restarted by the
// supervisor, producing multiple marker writes before the crash-loop guard fails it.
use servicio_core::process::TokioProcess;
use servicio_core::supervisor::InstanceSupervisor;
use servicio_core::worker::{RestartKind, RestartPolicy, RunMode, WorkerSpec};
use std::collections::BTreeMap;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn crashing_worker_is_restarted_until_crash_loop_guard() {
    let dir = tempdir().unwrap();
    let counter = dir.path().join("count");

    let spec = WorkerSpec {
        name: "crasher".into(),
        command: "sh".into(),
        args: vec![
            "-c".into(),
            format!("echo x >> {} ; exit 1", counter.display()),
        ],
        working_dir: dir.path().to_path_buf(),
        env: BTreeMap::new(),
        run_mode: RunMode::Daemon { concurrency: 1 },
        restart: RestartPolicy {
            kind: RestartKind::OnFailure,
            max_retries: 3,
            base_secs: 0,
            max_secs: 0,
            reset_window_secs: 3600,
        },
        autostart: true,
        enabled: true,
    };

    let sup = InstanceSupervisor::new(0, spec, Arc::new(TokioProcess), dir.path().join("c.log"));
    sup.run_until_terminal().await;

    // initial run + 3 retries = 4 executions = 4 marker lines.
    let body = std::fs::read_to_string(&counter).unwrap();
    assert_eq!(body.lines().count(), 4);
    assert_eq!(sup.restart_count(), 3);
}
```

- [ ] **Step 2: Run it to verify it fails (before this point the binary is built but assert may differ)**

Run: `cargo test -p servicio-core --test restart_integration`
Expected: PASS if Task 6 is correct. If it FAILS on the line count, the off-by-one is in `should_retry` — confirm `backoff.is_crash_loop(retries + 1)` semantics match Task 1 (`is_crash_loop` trips when arg > max_retries).

> This test is the regression anchor for the whole phase. It must stay green.

- [ ] **Step 3: Commit**

```bash
git add crates/servicio-core/tests/restart_integration.rs
git commit -m "test(core): end-to-end crash-and-restart integration test"
```

---

## Task 11: README + run instructions

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write the README**

Create `README.md`:

```markdown
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

```bash
# add an always-on worker
cargo run -p servicio-daemon -- --db servicio.db \
  add --name ticker --command sh --args "-c 'while true; do echo tick; sleep 1; done'" --concurrency 2

# list workers
cargo run -p servicio-daemon -- --db servicio.db list

# supervise autostart workers until Ctrl-C (logs in $TMPDIR/servicio-logs)
cargo run -p servicio-daemon -- --db servicio.db run
```

### Crates
- `servicio-core` — supervisor engine (pure, fully unit-tested).
- `servicio-daemon` — persistence + CLI.

Next phases: IPC + Tauri GUI, scheduled/batch modes, framework autodetect, packaging.
```

- [ ] **Step 2: Verify the documented commands work**

Run: `cargo test && cargo run -p servicio-daemon -- --db /tmp/servicio-readme.db list`
Expected: tests PASS; `list` runs without error (empty output on a fresh db).

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: phase 1 readme and run instructions"
```

---

## Definition of Done (Phase 1)

- `cargo test` is green across the workspace (unit + 2 integration tests).
- `servicio-core` has no rusqlite/clap dependency (engine stays pure).
- A crashing worker is observably restarted with exponential backoff and stopped by the
  crash-loop guard (Task 10).
- Worker definitions persist in SQLite and reconcile (autostart filter) on reload (Tasks 8–9).
- Logs are captured to rotating per-instance files (Task 5, used by Task 6).
- README documents build/test/run.

## Out of scope (later phases / plans)
- Scheduled + batch run modes (new `RunMode` variants + scheduler).
- IPC server (Unix socket / named pipe) + auth token.
- Tauri GUI (dashboard, detail, wizard).
- OS-service install (launchd / Windows Service / systemd) + run-on-boot.
- Graceful SIGTERM-then-SIGKILL stop, health checks, metrics sampling, notifications.
- Config import/export (`servicio.yaml`) + framework detectors.
- Packaging, signing, updater.
```

---

## Implementation deviations (recorded during execution)

All 11 tasks were implemented and committed; `cargo test` = 27 passing. Deviations from
the original task text, each justified:
- **Task 0 / 8 — workspace members:** the root `Cargo.toml` initially declares only
  `crates/servicio-core` as a member; Task 8 adds `crates/servicio-daemon`. Cargo refuses
  to load a workspace whose declared member directory does not yet exist, so `cargo build
  -p servicio-core` cannot pass in Task 0 if the daemon is declared early.
- **Task 4 — `Spawned: Debug`:** a manual `impl Debug for Spawned` was added because the
  test's `unwrap_err()` requires the `Ok` type to be `Debug`, and `Child` / boxed
  `dyn AsyncRead` cannot derive it.
- **Task 6 — concurrent log pump (post-review fix, commit `2545d87`):** the original code
  drained stdout to EOF *before* calling `wait()` (sequential). It was rewritten to drain
  stdout AND stderr concurrently with `wait()` via a spawned pump task bounded by a 2s
  join-then-abort, so a long-running worker (or one whose grandchild holds the pipe open)
  cannot wedge exit detection, and a chatty stderr cannot fill its pipe buffer and stall.
  The spawn-failure path now emits `Crashed` before `Backoff` (legal transition). A
  `captures_stderr_to_log` regression test was added. Known Phase-1 limitation documented
  in-code: the single-sample `reset_window` can let a worker that always crashes just past
  the window evade the crash-loop guard (systemd-style start-limit semantics); a
  sliding-window counter replaces it in a later phase.
- **Task 9 — `--args` parsing (post-review fix, commit `faed7fe`):** the test CLI's
  `--args` uses `num_args = 0.., allow_hyphen_values = true` (not `value_delimiter = ' '`)
  so values like `-c` are accepted and quoting is preserved. Consequence: `--args` must be
  the LAST flag on the command line (greedy trailing var-arg). The README documents this.

## Self-review notes (addressed inline)
- **Spec coverage:** This plan implements the Phase-1 slice of the spec's §3–§6 (daemon
  process model, daemon run mode, restart/backoff/crash-loop, log capture+rotation,
  SQLite persistence, reconcile). Scheduled/batch (§4), IPC/security (§8), GUI (§7), and
  packaging (§9) are explicitly deferred and listed under "Out of scope."
- **Type consistency:** `WorkerSpec`, `RunMode::Daemon { concurrency }`, `RestartPolicy`
  (with `base_secs`/`max_secs`/`reset_window_secs`/`max_retries`/`kind`), `InstanceState`,
  `Backoff` (`delay_for_attempt`/`is_crash_loop`/`should_reset`), `ProcessSpawner`/
  `Spawned`/`TokioProcess`, `LogSink::write_line`, `InstanceSupervisor::run_until_terminal`/
  `restart_count`/`subscribe`, `Manager::start_worker`/`instance_count`/`stop_all`, and
  `Db::open`/`open_in_memory`/`upsert_worker`/`list_workers`/`autostart_workers` are used
  consistently across tasks.
- **Crash-loop math:** `is_crash_loop(n)` trips when `n > max_retries`; supervisor calls
  `should_retry` = `!is_crash_loop(retries + 1)`, so with `max_retries = k` you get the
  initial run + `k` retries = `k+1` executions, matching Task 10's assertion.
