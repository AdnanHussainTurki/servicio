use servicio_cli_lib::Client;
use servicio_core::worker::{RestartPolicy, RunMode, WorkerSpec};
use servicio_daemon_lib::paths::Paths;
use servicio_daemon_lib::serve::serve;
use std::collections::BTreeMap;
use std::time::Duration;

#[tokio::test]
async fn client_handshakes_and_lists_after_add() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let handle = serve(paths.clone(), "secret".into()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut client = Client::connect(&paths.base, "secret").await.unwrap();

    let spec = WorkerSpec {
        name: "q".into(),
        command: "sh".into(),
        args: vec!["-c".into(), "sleep 30".into()],
        working_dir: std::path::PathBuf::from("/"),
        env: BTreeMap::new(),
        run_mode: RunMode::Daemon { concurrency: 1 },
        restart: RestartPolicy::default(),
        autostart: false,
        enabled: true,
        group: None,
        tags: Vec::new(),
        display_name: None,
    };
    client.add_worker(&spec).await.unwrap();
    let list = client.list_workers().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "q");

    handle.shutdown().await;
}
