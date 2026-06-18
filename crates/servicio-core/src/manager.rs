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
        if let Some(old) = self.workers.remove(&spec.name) {
            for h in old.handles {
                h.abort();
            }
        }
        let concurrency = match spec.run_mode {
            RunMode::Daemon { concurrency } => concurrency.max(1),
            _ => 1, // scheduled/batch refined in a later task
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
                        index: s.index(),
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

    /// Stop one worker's instances (awaiting teardown). Returns true if it existed.
    pub async fn stop_worker(&mut self, name: &str) -> bool {
        if let Some(w) = self.workers.remove(name) {
            for h in w.handles {
                h.abort();
                let _ = h.await;
            }
            true
        } else {
            false
        }
    }

    /// Stop all workers and await their teardown so child processes are reaped.
    pub async fn stop_all(&mut self) {
        for (_, w) in self.workers.drain() {
            for h in w.handles {
                h.abort();
                let _ = h.await;
            }
        }
    }
}

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

    #[tokio::test]
    async fn status_reports_running_instances() {
        let dir = tempfile::tempdir().unwrap();
        let mut mgr = Manager::new(Arc::new(TokioProcess), dir.path().to_path_buf());
        mgr.start_worker(long_running("q")).await;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let status = mgr.status();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].name, "q");
        assert_eq!(status[0].instances.len(), 2);
        mgr.stop_all().await;
    }

    #[tokio::test]
    async fn restarting_worker_does_not_leak_instances() {
        let dir = tempfile::tempdir().unwrap();
        let mut mgr = Manager::new(Arc::new(TokioProcess), dir.path().to_path_buf());
        mgr.start_worker(long_running("q")).await;
        mgr.start_worker(long_running("q")).await; // restart, must replace not accumulate
        assert_eq!(mgr.instance_count("q"), 2);
        mgr.stop_all().await;
    }

    #[tokio::test]
    async fn subscribe_receives_state_events() {
        use crate::event::SupervisorEvent;
        let dir = tempfile::tempdir().unwrap();
        let mut mgr = Manager::new(Arc::new(TokioProcess), dir.path().to_path_buf());
        let mut rx = mgr.subscribe();
        mgr.start_worker(long_running("q")).await;
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
}
