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
        group: None,
        tags: Vec::new(),
        display_name: None,
    };

    let sup = InstanceSupervisor::new(0, spec, Arc::new(TokioProcess), dir.path().join("c.log"));
    sup.run_until_terminal().await;

    // initial run + 3 retries = 4 executions = 4 marker lines.
    let body = std::fs::read_to_string(&counter).unwrap();
    assert_eq!(body.lines().count(), 4);
    assert_eq!(sup.restart_count(), 3);
}
