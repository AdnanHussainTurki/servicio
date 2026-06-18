use crate::backoff::Backoff;
use crate::event::SupervisorEvent;
use crate::logsink::LogSink;
use crate::process::ProcessSpawner;
use crate::state::InstanceState;
use crate::worker::{OverlapPolicy, RestartKind, RunMode, Schedule, WorkerSpec};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::broadcast;
use tokio::sync::watch;

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
    events: Option<broadcast::Sender<SupervisorEvent>>,
    pid: AtomicU32,
}

impl InstanceSupervisor {
    pub fn new(
        index: u32,
        spec: WorkerSpec,
        spawner: Arc<dyn ProcessSpawner>,
        log_path: PathBuf,
    ) -> Self {
        let (state_tx, state_rx) = watch::channel(InstanceState::Stopped);
        Self {
            index,
            spec,
            spawner,
            log_path,
            restarts: AtomicU32::new(0),
            state_tx,
            state_rx,
            events: None,
            pid: AtomicU32::new(0),
        }
    }

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

    /// This instance's index within its worker.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Worker name this instance belongs to.
    pub fn worker_name(&self) -> &str {
        &self.spec.name
    }

    pub fn subscribe(&self) -> watch::Receiver<InstanceState> {
        self.state_rx.clone()
    }

    pub fn restart_count(&self) -> u32 {
        self.restarts.load(Ordering::SeqCst)
    }

    fn set_state(&self, s: InstanceState) {
        let from = *self.state_rx.borrow();
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

    fn backoff(&self) -> Backoff {
        let r = &self.spec.restart;
        Backoff::new(
            Duration::from_secs(r.base_secs),
            Duration::from_secs(r.max_secs),
            r.max_retries,
            Duration::from_secs(r.reset_window_secs),
        )
    }

    /// Dispatch to the appropriate run loop based on this worker's run mode.
    pub async fn run_until_terminal(&self) {
        match self.spec.run_mode.clone() {
            RunMode::Daemon { .. } => self.run_daemon().await,
            RunMode::Scheduled { schedule, overlap } => self.run_scheduled(&schedule, overlap).await,
            RunMode::Batch { run_count, delay_secs } => self.run_batch(run_count, delay_secs).await,
        }
    }

    /// Spawn the process once, pump stdout/stderr concurrently, wait for exit.
    ///
    /// Sets `Starting → Running` (or `Crashed` on spawn failure) and tracks pid.
    /// Does NOT decide a post-exit terminal state — the caller (daemon/batch/
    /// scheduled loop) owns restart/completion bookkeeping. Returns true on a
    /// successful exit (status 0), false on failure or spawn error.
    async fn run_once(&self, sink: &std::sync::Arc<std::sync::Mutex<LogSink>>) -> bool {
        self.set_state(InstanceState::Starting);

        let mut spawned = match self.spawner.spawn(&self.spec) {
            Ok(s) => s,
            Err(_) => {
                self.set_state(InstanceState::Crashed);
                return false;
            }
        };
        self.set_state(InstanceState::Running);
        self.pid.store(spawned.pid().unwrap_or(0), Ordering::SeqCst);

        // Drain stdout AND stderr concurrently with waiting for exit, so a
        // long-running worker that holds its pipes open does not block exit
        // detection, and a chatty stderr cannot fill its pipe buffer and stall.
        let idx = self.index;
        let out = spawned.stdout.take();
        let err = spawned.stderr.take();
        let sink_out = Arc::clone(sink);
        let sink_err = Arc::clone(sink);
        let ev = self.events.clone();
        let name = self.spec.name.clone();
        let mut pump = tokio::spawn(async move {
            let out_ev = ev.clone();
            let out_name = name.clone();
            let out_fut = async move {
                if let Some(o) = out {
                    let mut lines = BufReader::new(o).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = sink_out.lock().unwrap().write_line(idx, "stdout", &line);
                        if let Some(tx) = &out_ev {
                            let _ = tx.send(SupervisorEvent::Log {
                                worker: out_name.clone(),
                                instance: idx,
                                stream: "stdout".into(),
                                line: line.clone(),
                            });
                        }
                    }
                }
            };
            let err_ev = ev.clone();
            let err_name = name.clone();
            let err_fut = async move {
                if let Some(e) = err {
                    let mut lines = BufReader::new(e).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = sink_err.lock().unwrap().write_line(idx, "stderr", &line);
                        if let Some(tx) = &err_ev {
                            let _ = tx.send(SupervisorEvent::Log {
                                worker: err_name.clone(),
                                instance: idx,
                                stream: "stderr".into(),
                                line: line.clone(),
                            });
                        }
                    }
                }
            };
            tokio::join!(out_fut, err_fut);
        });

        let status = spawned.wait().await;
        self.pid.store(0, Ordering::SeqCst);

        // Normally the pipes hit EOF when the child exits and the pump ends on
        // its own. Bound the join so a lingering grandchild holding the pipe
        // open cannot wedge the supervisor; then stop pumping.
        if tokio::time::timeout(Duration::from_secs(2), &mut pump).await.is_err() {
            pump.abort();
        }

        status.map(|s| s.success()).unwrap_or(false)
    }

    /// Run the spawn/monitor/restart loop until the instance reaches a terminal state.
    async fn run_daemon(&self) {
        let backoff = self.backoff();
        let mut retries: u32 = 0;
        let sink = match LogSink::new(&self.log_path, 10 * 1024 * 1024, 5) {
            Ok(s) => Arc::new(std::sync::Mutex::new(s)),
            Err(e) => {
                tracing::error!("failed to open log sink at {:?}: {e}", self.log_path);
                self.set_state(InstanceState::Failed);
                return;
            }
        };

        loop {
            // Measure uptime around run_once so the reset-window logic (which keys
            // off how long the process actually ran) keeps its daemon semantics.
            let started = Instant::now();
            let success = self.run_once(&sink).await;
            let uptime = started.elapsed();

            // Reset the retry counter after a sufficiently long run (systemd-style
            // start-limit window). NOTE (Phase 2): a worker that always crashes just
            // after this window can evade the crash-loop guard; a sliding-window
            // counter will replace this single-sample check later.
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

    /// Run on a schedule: idle → wait for next fire → run once → repeat.
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
            // v1: Skip semantics — each run is awaited before the next is scheduled.
            // Queue/KillPrevious accepted in the type but behave as Skip for now.
            let _ = overlap;
            self.run_once(&sink).await;
        }
    }

    /// Run a fixed number of times with an optional inter-run delay, then settle
    /// into a terminal Completed/Failed state.
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

        let mut seen = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            if let SupervisorEvent::State { from, to, .. } = ev {
                seen.push((from, to));
            }
        }
        assert!(seen.contains(&(InstanceState::Starting, InstanceState::Running)));
        assert!(seen.contains(&(InstanceState::Running, InstanceState::Stopped)));
    }

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

    #[tokio::test]
    async fn captures_stderr_to_log() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("t.log");
        let policy = RestartPolicy { kind: RestartKind::Never, ..Default::default() };
        let sup = InstanceSupervisor::new(
            0,
            spec("sh", &["-c", "echo oops 1>&2"], policy),
            Arc::new(TokioProcess),
            log.clone(),
        );
        sup.run_until_terminal().await;
        let contents = std::fs::read_to_string(&log).unwrap();
        assert!(contents.contains("[stderr]"), "log was: {contents}");
        assert!(contents.contains("oops"));
    }
}
