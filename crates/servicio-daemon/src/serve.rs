use crate::db::Db;
use crate::paths::Paths;
use servicio_core::manager::Manager;
use servicio_core::process::TokioProcess;
use servicio_ipc::Frame;
use serde_json::json;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::watch;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Shared daemon state handed to each connection.
pub struct Daemon {
    pub token: String,
    pub manager: Mutex<Manager>,
    pub db: Mutex<Db>,
    pub started: std::time::Instant,
    pub version: String,
}

/// Handle to a running server; used to stop it.
pub struct ServeHandle {
    shutdown_tx: watch::Sender<bool>,
    accept_task: JoinHandle<()>,
    socket_path: std::path::PathBuf,
}

impl ServeHandle {
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        let _ = self.accept_task.await;
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Bind the socket and start accepting connections in the background.
/// Reconciles autostart workers from the DB before returning.
pub async fn serve(paths: Paths, token: String) -> std::io::Result<ServeHandle> {
    std::fs::create_dir_all(&paths.base)?;
    let _ = std::fs::remove_file(paths.socket());
    let listener = UnixListener::bind(paths.socket())?;
    set_socket_perms(&paths.socket())?;

    let db = Db::open(&paths.db()).map_err(to_io)?;
    let log_dir = paths.base.join("logs");
    let mut manager = Manager::new(Arc::new(TokioProcess), log_dir);
    for spec in db.autostart_workers().map_err(to_io)? {
        manager.start_worker(spec).await;
    }

    let daemon = Arc::new(Daemon {
        token,
        manager: Mutex::new(manager),
        db: Mutex::new(db),
        started: std::time::Instant::now(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    });

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let socket_path = paths.socket();
    let accept_task = tokio::spawn(accept_loop(listener, daemon, shutdown_rx));
    Ok(ServeHandle { shutdown_tx, accept_task, socket_path })
}

async fn accept_loop(
    listener: UnixListener,
    daemon: Arc<Daemon>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() { break; }
            }
            accepted = listener.accept() => {
                if let Ok((stream, _addr)) = accepted {
                    let d = Arc::clone(&daemon);
                    tokio::spawn(handle_conn(stream, d));
                }
            }
        }
    }
}

async fn handle_conn(stream: UnixStream, daemon: Arc<Daemon>) {
    let (rd, mut wr) = stream.into_split();
    let mut lines = BufReader::new(rd).lines();
    let mut authed = false;

    while let Ok(Some(line)) = lines.next_line().await {
        let frame = match Frame::from_line(&line) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let Frame::Request { id, method, params } = frame else { continue };

        if !authed {
            if method == "hello"
                && params.get("token").and_then(|t| t.as_str()) == Some(daemon.token.as_str())
            {
                authed = true;
                let _ = write_frame(&mut wr, &Frame::ok(id, json!({"daemon_version": daemon.version}))).await;
                continue;
            }
            let _ = write_frame(&mut wr, &Frame::err(id, "unauthorized", "valid hello required")).await;
            return;
        }

        let reply = dispatch(&daemon, id, &method, params).await;
        if write_frame(&mut wr, &reply).await.is_err() {
            return;
        }
    }
}

/// Method dispatch for authenticated connections. Extended in Task 6/7.
async fn dispatch(daemon: &Arc<Daemon>, id: u64, method: &str, _params: serde_json::Value) -> Frame {
    match method {
        "ping" => Frame::ok(id, json!({"pong": true})),
        "daemon_info" => {
            let mgr = daemon.manager.lock().await;
            let status = mgr.status();
            let worker_count = status.len() as u32;
            let running_count = status
                .iter()
                .filter(|w| w.instances.iter().any(|i| matches!(i.state, servicio_core::state::InstanceState::Running)))
                .count() as u32;
            Frame::ok(
                id,
                json!({
                    "version": daemon.version,
                    "uptime_secs": daemon.started.elapsed().as_secs(),
                    "worker_count": worker_count,
                    "running_count": running_count,
                }),
            )
        }
        other => Frame::err(id, "unknown_method", &format!("no such method: {other}")),
    }
}

async fn write_frame(wr: &mut tokio::net::unix::OwnedWriteHalf, frame: &Frame) -> std::io::Result<()> {
    wr.write_all(format!("{}\n", frame.to_line()).as_bytes()).await
}

#[cfg(unix)]
fn set_socket_perms(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_socket_perms(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

fn to_io<E: std::fmt::Display>(e: E) -> std::io::Error {
    std::io::Error::other(e.to_string())
}
