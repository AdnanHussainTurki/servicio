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
