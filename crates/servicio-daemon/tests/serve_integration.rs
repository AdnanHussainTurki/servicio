// Spin up the server on a temp socket in-process, connect a raw client, and
// exercise the handshake + ping + shutdown.
use servicio_daemon_lib::paths::Paths;
use servicio_daemon_lib::serve::{serve, ServeHandle};
use servicio_core::worker::{RestartPolicy, RunMode, WorkerSpec};
use servicio_ipc::Frame;
use serde_json::json;
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

async fn start(paths: Paths, token: String) -> ServeHandle {
    let h = serve(paths, token).await.expect("serve starts");
    tokio::time::sleep(Duration::from_millis(100)).await;
    h
}

async fn send_recv(sock: &std::path::Path, frames: &[Frame]) -> Vec<Frame> {
    let stream = UnixStream::connect(sock).await.unwrap();
    let (rd, mut wr) = stream.into_split();
    for f in frames {
        wr.write_all(format!("{}\n", f.to_line()).as_bytes()).await.unwrap();
    }
    let mut lines = BufReader::new(rd).lines();
    let mut out = Vec::new();
    for _ in 0..frames.len() {
        if let Ok(Some(line)) = lines.next_line().await {
            out.push(Frame::from_line(&line).unwrap());
        }
    }
    out
}

#[tokio::test]
async fn good_token_then_ping_works() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;
    let replies = send_recv(
        &paths.socket(),
        &[
            Frame::Request { id: 1, method: "hello".into(), params: json!({"token": "secret"}) },
            Frame::Request { id: 2, method: "ping".into(), params: json!({}) },
        ],
    )
    .await;
    assert!(matches!(replies[0], Frame::Response { id: 1, error: None, .. }));
    match &replies[1] {
        Frame::Response { id: 2, result: Some(v), .. } => assert_eq!(v["pong"], true),
        other => panic!("unexpected: {other:?}"),
    }
    h.shutdown().await;
}

#[tokio::test]
async fn bad_token_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;
    let replies = send_recv(
        &paths.socket(),
        &[Frame::Request { id: 1, method: "hello".into(), params: json!({"token": "wrong"}) }],
    )
    .await;
    match &replies[0] {
        Frame::Response { id: 1, error: Some(e), .. } => assert_eq!(e.code, "unauthorized"),
        other => panic!("expected unauthorized, got {other:?}"),
    }
    h.shutdown().await;
}

#[tokio::test]
async fn shutdown_removes_socket() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;
    assert!(paths.socket().exists());
    h.shutdown().await;
    assert!(!paths.socket().exists());
}

async fn hello_then(sock: &std::path::Path, reqs: Vec<Frame>) -> Vec<Frame> {
    let mut frames = vec![Frame::Request { id: 0, method: "hello".into(), params: json!({"token":"secret"}) }];
    frames.extend(reqs);
    send_recv(sock, &frames).await
}

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
        group: None,
        tags: Vec::new(),
    }
}

#[tokio::test]
async fn add_then_list_reflects_worker() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;
    let replies = hello_then(
        &paths.socket(),
        vec![
            Frame::Request { id: 1, method: "add_worker".into(), params: json!({ "spec": sleeper("q") }) },
            Frame::Request { id: 2, method: "list_workers".into(), params: json!({}) },
        ],
    )
    .await;
    match &replies[2] {
        Frame::Response { id: 2, result: Some(v), .. } => {
            let arr = v.as_array().unwrap();
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0]["name"], "q");
        }
        other => panic!("unexpected list reply: {other:?}"),
    }
    h.shutdown().await;
}

#[tokio::test]
async fn start_then_stop_worker() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;
    let replies = hello_then(
        &paths.socket(),
        vec![
            Frame::Request { id: 1, method: "add_worker".into(), params: json!({ "spec": sleeper("q") }) },
            Frame::Request { id: 2, method: "start_worker".into(), params: json!({"name":"q"}) },
            Frame::Request { id: 3, method: "stop_worker".into(), params: json!({"name":"q"}) },
        ],
    )
    .await;
    assert!(matches!(replies[2], Frame::Response { id: 2, error: None, .. }));
    assert!(matches!(replies[3], Frame::Response { id: 3, error: None, .. }));
    h.shutdown().await;
}

#[tokio::test]
async fn oversized_line_does_not_crash_server() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;

    // One connection sends a 2 MiB line with no newline; server should drop it.
    {
        let stream = UnixStream::connect(&paths.socket()).await.unwrap();
        let (_rd, mut wr) = stream.into_split();
        let big = vec![b'x'; 2 * 1024 * 1024];
        let _ = wr.write_all(&big).await;
        let _ = wr.flush().await;
    }
    // A fresh connection must still work — proves the daemon is alive.
    let replies = send_recv(
        &paths.socket(),
        &[Frame::Request { id: 1, method: "hello".into(), params: json!({"token":"secret"}) }],
    )
    .await;
    assert!(matches!(replies[0], Frame::Response { id: 1, error: None, .. }));
    h.shutdown().await;
}

#[tokio::test]
async fn subscribe_streams_state_events_for_started_worker() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;

    // Register the worker (persist only).
    let _ = hello_then(
        &paths.socket(),
        vec![Frame::Request { id: 1, method: "add_worker".into(), params: json!({ "spec": sleeper("q") }) }],
    )
    .await;

    // Subscriber connection: hello + subscribe, consume the two acks.
    let stream = UnixStream::connect(&paths.socket()).await.unwrap();
    let (rd, mut wr) = stream.into_split();
    for f in [
        Frame::Request { id: 0, method: "hello".into(), params: json!({"token":"secret"}) },
        Frame::Request { id: 1, method: "subscribe".into(), params: json!({"topics":["state"]}) },
    ] {
        wr.write_all(format!("{}\n", f.to_line()).as_bytes()).await.unwrap();
    }
    let mut lines = BufReader::new(rd).lines();
    let _ = lines.next_line().await.unwrap(); // hello ack
    let _ = lines.next_line().await.unwrap(); // subscribe ack

    // Trigger a start on a separate connection.
    let _ = hello_then(
        &paths.socket(),
        vec![Frame::Request { id: 1, method: "start_worker".into(), params: json!({"name":"q"}) }],
    )
    .await;

    // Expect a state Event within a couple seconds.
    let got = tokio::time::timeout(Duration::from_secs(3), async {
        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(Frame::Event { topic, .. }) = Frame::from_line(&line) {
                if topic == "state" { return true; }
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(got, "expected a state event after start");
    h.shutdown().await;
}

#[tokio::test]
async fn shutdown_stops_worker_children() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;

    let marker = dir.path().join("alive");
    let mut spec = sleeper("q");
    spec.args = vec![
        "-c".into(),
        format!("while true; do echo x >> {} ; sleep 0.05; done", marker.display()),
    ];

    let _ = hello_then(
        &paths.socket(),
        vec![
            Frame::Request { id: 1, method: "add_worker".into(), params: json!({ "spec": spec }) },
            Frame::Request { id: 2, method: "start_worker".into(), params: json!({"name":"q"}) },
        ],
    )
    .await;

    // Let the child run and grow the marker file.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(marker.exists(), "worker should have started writing");

    h.shutdown().await;

    // After shutdown the child must be dead: file size stops changing.
    tokio::time::sleep(Duration::from_millis(150)).await;
    let size_a = std::fs::metadata(&marker).map(|m| m.len()).unwrap_or(0);
    tokio::time::sleep(Duration::from_millis(400)).await;
    let size_b = std::fs::metadata(&marker).map(|m| m.len()).unwrap_or(0);
    assert_eq!(size_a, size_b, "worker child kept running after shutdown (orphaned)");
}

#[tokio::test]
async fn metrics_method_returns_series_for_running_worker() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;
    let spec = sleeper("q");
    let _ = hello_then(&paths.socket(), vec![
        Frame::Request { id: 1, method: "add_worker".into(), params: json!({"spec": spec}) },
        Frame::Request { id: 2, method: "start_worker".into(), params: json!({"name":"q"}) },
    ]).await;
    tokio::time::sleep(std::time::Duration::from_millis(5000)).await; // >=2 sampler ticks
    let replies = hello_then(&paths.socket(), vec![
        Frame::Request { id: 1, method: "metrics".into(), params: json!({"worker":"q","since_secs":3600}) },
    ]).await;
    match &replies[1] {
        Frame::Response { id: 1, result: Some(v), .. } => {
            let arr = v.as_array().unwrap();
            assert!(!arr.is_empty(), "expected >=1 instance series");
            assert!(arr[0]["points"].as_array().unwrap().len() >= 1, "expected >=1 sample point");
        }
        other => panic!("unexpected: {other:?}"),
    }
    h.shutdown().await;
}

#[tokio::test]
async fn daemon_log_method_returns_log_field() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    // pre-write a log file so the daemon has something to tail
    std::fs::write(dir.path().join("daemon.log"), "line one\nline two\n").unwrap();
    let h = start(paths.clone(), "secret".into()).await;
    let replies = hello_then(&paths.socket(), vec![
        Frame::Request { id: 1, method: "daemon_log".into(), params: json!({"lines": 10}) },
    ]).await;
    match &replies[1] {
        Frame::Response { id: 1, result: Some(v), .. } => {
            assert!(v.get("log").is_some());
            assert!(v["log"].as_str().unwrap().contains("line two"));
        }
        other => panic!("unexpected: {other:?}"),
    }
    h.shutdown().await;
}

#[tokio::test]
async fn get_worker_returns_full_spec() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;
    let _ = hello_then(&paths.socket(), vec![
        Frame::Request { id: 1, method: "add_worker".into(), params: json!({"spec": sleeper("q")}) },
    ]).await;
    let replies = hello_then(&paths.socket(), vec![
        Frame::Request { id: 1, method: "get_worker".into(), params: json!({"name":"q"}) },
    ]).await;
    match &replies[1] {
        Frame::Response { id: 1, result: Some(v), .. } => assert_eq!(v["name"], "q"),
        other => panic!("unexpected: {other:?}"),
    }
    h.shutdown().await;
}

#[tokio::test]
async fn daemon_info_includes_build_and_shutdown_acks() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;
    let replies = hello_then(&paths.socket(), vec![
        Frame::Request { id: 1, method: "daemon_info".into(), params: json!({}) },
        Frame::Request { id: 2, method: "shutdown".into(), params: json!({}) },
    ]).await;
    match &replies[1] {
        Frame::Response { id: 1, result: Some(v), .. } => assert!(v.get("build").is_some()),
        o => panic!("{o:?}"),
    }
    match &replies[2] {
        Frame::Response { id: 2, result: Some(v), .. } => assert_eq!(v["shutting_down"], true),
        o => panic!("{o:?}"),
    }
    h.shutdown().await;
}

fn grouped(name: &str, group: &str) -> WorkerSpec {
    let mut s = sleeper(name);
    s.group = Some(group.to_string());
    s
}

#[tokio::test]
async fn start_group_starts_all_workers_in_group() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;
    let _ = hello_then(&paths.socket(), vec![
        Frame::Request { id: 1, method: "add_worker".into(), params: json!({"spec": grouped("a","billing")}) },
        Frame::Request { id: 2, method: "add_worker".into(), params: json!({"spec": grouped("b","billing")}) },
        Frame::Request { id: 3, method: "add_worker".into(), params: json!({"spec": grouped("c","other")}) },
    ]).await;
    let replies = hello_then(&paths.socket(), vec![
        Frame::Request { id: 1, method: "start_group".into(), params: json!({"group":"billing"}) },
        Frame::Request { id: 2, method: "list_workers".into(), params: json!({}) },
        Frame::Request { id: 3, method: "stop_group".into(), params: json!({"group":"billing"}) },
    ]).await;
    match &replies[1] { Frame::Response { id:1, result: Some(v), .. } => assert_eq!(v["started"], 2), o => panic!("{o:?}") }
    match &replies[3] { Frame::Response { id:3, result: Some(v), .. } => assert_eq!(v["stopped"], 2), o => panic!("{o:?}") }
    h.shutdown().await;
}

#[tokio::test]
async fn export_then_import_workers_roundtrips() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;
    let _ = hello_then(&paths.socket(), vec![
        Frame::Request { id: 1, method: "add_worker".into(), params: json!({"spec": sleeper("q")}) },
    ]).await;
    let exp = hello_then(&paths.socket(), vec![
        Frame::Request { id: 1, method: "export_workers".into(), params: json!({}) },
    ]).await;
    let workers = match &exp[1] { Frame::Response { id:1, result: Some(v), .. } => v["workers"].clone(), o => panic!("{o:?}") };
    assert_eq!(workers.as_array().unwrap().len(), 1);
    // import into a fresh daemon
    let dir2 = tempfile::tempdir().unwrap();
    let paths2 = Paths::new(dir2.path().to_path_buf());
    let h2 = start(paths2.clone(), "secret".into()).await;
    let imp = hello_then(&paths2.socket(), vec![
        Frame::Request { id: 1, method: "import_workers".into(), params: json!({"workers": workers}) },
        Frame::Request { id: 2, method: "list_workers".into(), params: json!({}) },
    ]).await;
    match &imp[1] { Frame::Response { id:1, result: Some(v), .. } => assert_eq!(v["imported"], 1), o => panic!("{o:?}") }
    match &imp[2] { Frame::Response { id:2, result: Some(v), .. } => assert_eq!(v.as_array().unwrap().len(), 1), o => panic!("{o:?}") }
    h.shutdown().await; h2.shutdown().await;
}

#[tokio::test]
async fn detect_workers_finds_laravel_in_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;
    let proj = dir.path().join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::write(proj.join("artisan"), "#!/usr/bin/env php").unwrap();
    let replies = hello_then(&paths.socket(), vec![
        Frame::Request { id: 1, method: "detect_workers".into(), params: json!({"path": proj.to_str().unwrap()}) },
    ]).await;
    match &replies[1] {
        Frame::Response { id: 1, result: Some(v), .. } => {
            let arr = v.as_array().unwrap();
            assert!(arr.iter().any(|s| s["source"] == "laravel/artisan"));
            assert!(arr.iter().any(|s| s["source"] == "generic"));
        }
        other => panic!("unexpected: {other:?}"),
    }
    h.shutdown().await;
}
