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
