use crate::process::ProcessSpawner;
use crate::supervisor::InstanceSupervisor;
use crate::worker::{RunMode, WorkerSpec};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinHandle;

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
