use servicio_daemon_lib::paths::Paths;
use servicio_daemon_lib::serve::serve;
use std::time::Duration;
use servicio_app::bridge;
use servicio_app::state::AppState;

async fn running_daemon(dir: &std::path::Path) -> (Paths, servicio_daemon_lib::serve::ServeHandle, AppState) {
    let paths = Paths::new(dir.to_path_buf());
    let handle = serve(paths.clone(), "secret".into()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let state = AppState::connect(&paths.socket(), "secret").await.unwrap();
    (paths, handle, state)
}

#[tokio::test]
async fn daemon_status_reports_connected() {
    let dir = tempfile::tempdir().unwrap();
    let (_p, handle, state) = running_daemon(dir.path()).await;
    let status = bridge::daemon_status(&state).await.unwrap();
    assert!(status.connected);
    handle.shutdown().await;
}

use servicio_core::worker::{RestartPolicy, RunMode, WorkerSpec};
use std::collections::BTreeMap;

fn sleeper(name: &str) -> WorkerSpec {
    WorkerSpec {
        name: name.into(),
        command: "sh".into(),
        args: vec!["-c".into(), "sleep 30".into()],
        working_dir: std::path::PathBuf::from("/"),
        env: BTreeMap::new(),
        run_mode: RunMode::Daemon { concurrency: 1 },
        restart: RestartPolicy::default(),
        autostart: false,
        enabled: true,
    }
}

#[tokio::test]
async fn add_list_start_stop_via_bridge() {
    let dir = tempfile::tempdir().unwrap();
    let (_p, handle, state) = running_daemon(dir.path()).await;

    bridge::add_worker(&state, sleeper("q")).await.unwrap();
    let list = bridge::list_workers(&state).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "q");

    bridge::start_worker(&state, "q").await.unwrap();
    bridge::stop_worker(&state, "q").await.unwrap();
    bridge::restart_worker(&state, "q").await.unwrap();
    bridge::stop_worker(&state, "q").await.unwrap();

    handle.shutdown().await;
}

use servicio_app::events::event_payload;
use servicio_ipc::Frame;
use serde_json::json;

#[test]
fn maps_state_event_frame_to_payload() {
    let frame = Frame::Event {
        topic: "state".into(),
        payload: json!({"worker":"q","instance":0,"from":"starting","to":"running"}),
    };
    let p = event_payload(&frame).unwrap();
    assert_eq!(p["kind"], "state");
    assert_eq!(p["worker"], "q");
    assert_eq!(p["to"], "running");
}

#[test]
fn non_event_frame_maps_to_none() {
    let frame = Frame::Response { id: 1, result: None, error: None };
    assert!(event_payload(&frame).is_none());
}
