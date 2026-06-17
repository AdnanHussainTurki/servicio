// Spin up the server on a temp socket in-process, connect a raw client, and
// exercise the handshake + ping + shutdown.
use servicio_daemon_lib::paths::Paths;
use servicio_daemon_lib::serve::{serve, ServeHandle};
use servicio_ipc::Frame;
use serde_json::json;
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
