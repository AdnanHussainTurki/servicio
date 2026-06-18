use crate::db::Db;
use crate::paths::Paths;
use serde_json::json;
use servicio_core::event::SupervisorEvent;
use servicio_core::manager::Manager;
use servicio_core::process::TokioProcess;
use servicio_core::worker::WorkerSpec;
use servicio_ipc::types::{
    InstanceStatus as IpcInstanceStatus, LogEvent, StateEvent, WorkerStatus,
};
use servicio_ipc::Frame;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::watch;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

const MAX_LINE_BYTES: usize = 1024 * 1024; // 1 MiB per frame

/// Shared daemon state handed to each connection.
pub struct Daemon {
    pub token: String,
    pub manager: Mutex<Manager>,
    pub db: Mutex<Db>,
    pub started: std::time::Instant,
    pub version: String,
    pub build: String,
    pub log_path: std::path::PathBuf,
    pub shutdown: Arc<tokio::sync::Notify>,
}

/// Handle to a running server; used to stop it.
pub struct ServeHandle {
    shutdown_tx: watch::Sender<bool>,
    accept_task: JoinHandle<()>,
    socket_path: std::path::PathBuf,
    daemon: Arc<Daemon>,
    shutdown: Arc<tokio::sync::Notify>,
}

impl ServeHandle {
    /// A clone of the daemon's shutdown notifier; fires when a `shutdown` IPC
    /// is received so the host process can exit gracefully.
    pub fn shutdown_notify(&self) -> Arc<tokio::sync::Notify> {
        Arc::clone(&self.shutdown)
    }

    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        let _ = self.accept_task.await;
        self.daemon.manager.lock().await.stop_all().await;
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Bind the socket and start accepting connections in the background.
/// Reconciles autostart workers from the DB before returning.
pub async fn serve(paths: Paths, token: String) -> std::io::Result<ServeHandle> {
    std::fs::create_dir_all(&paths.base)?;
    set_dir_private(&paths.base)?;
    let _ = std::fs::remove_file(paths.socket());
    let listener = UnixListener::bind(paths.socket())?;
    set_socket_perms(&paths.socket())?;

    let db = Db::open(&paths.db()).map_err(to_io)?;
    let log_dir = paths.base.join("logs");
    let mut manager = Manager::new(Arc::new(TokioProcess), log_dir);
    for spec in db.autostart_workers().map_err(to_io)? {
        manager.start_worker(spec).await;
    }

    let shutdown = Arc::new(tokio::sync::Notify::new());
    let daemon = Arc::new(Daemon {
        token,
        manager: Mutex::new(manager),
        db: Mutex::new(db),
        started: std::time::Instant::now(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        build: env!("SERVICIO_BUILD").to_string(),
        log_path: paths.base.join("daemon.log"),
        shutdown: Arc::clone(&shutdown),
    });

    {
        let d = Arc::clone(&daemon);
        tokio::spawn(async move {
            crate::sampler::run_sampler_for(d, 3600).await;
        });
    }

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let socket_path = paths.socket();
    let accept_task = tokio::spawn(accept_loop(listener, Arc::clone(&daemon), shutdown_rx));
    Ok(ServeHandle {
        shutdown_tx,
        accept_task,
        socket_path,
        daemon,
        shutdown,
    })
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
    let (rd, wr) = stream.into_split();
    let wr = Arc::new(Mutex::new(wr));
    let mut reader = BufReader::new(rd);
    let mut authed = false;
    let mut buf = Vec::new();

    while let Ok(Some(())) = read_line_capped(&mut reader, &mut buf, MAX_LINE_BYTES).await {
        let line = match std::str::from_utf8(&buf) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let frame = match Frame::from_line(line) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let Frame::Request { id, method, params } = frame else {
            continue;
        };

        if !authed {
            if method == "hello"
                && params.get("token").and_then(|t| t.as_str()) == Some(daemon.token.as_str())
            {
                authed = true;
                let _ = write_frame_locked(
                    &wr,
                    &Frame::ok(id, json!({"daemon_version": daemon.version})),
                )
                .await;
                continue;
            }
            let _ =
                write_frame_locked(&wr, &Frame::err(id, "unauthorized", "valid hello required"))
                    .await;
            return;
        }

        if method == "subscribe" {
            let topics: Vec<String> = params
                .get("topics")
                .and_then(|t| t.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let worker_filter = params
                .get("worker")
                .and_then(|w| w.as_str())
                .map(String::from);
            let rx = daemon.manager.lock().await.subscribe();
            let _ = write_frame_locked(&wr, &Frame::ok(id, json!({"subscribed": true}))).await;
            spawn_forwarder(Arc::clone(&wr), rx, topics, worker_filter);
            continue;
        }

        let reply = dispatch(&daemon, id, &method, params).await;
        if write_frame_locked(&wr, &reply).await.is_err() {
            return;
        }
    }
}

fn spawn_forwarder(
    wr: Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
    mut rx: tokio::sync::broadcast::Receiver<SupervisorEvent>,
    topics: Vec<String>,
    worker_filter: Option<String>,
) {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let frame = match ev {
                        SupervisorEvent::State {
                            worker,
                            instance,
                            from,
                            to,
                        } => {
                            if !topics.iter().any(|t| t == "state") {
                                continue;
                            }
                            if let Some(f) = &worker_filter {
                                if f != &worker {
                                    continue;
                                }
                            }
                            Frame::Event {
                                topic: "state".into(),
                                payload: serde_json::to_value(StateEvent {
                                    worker,
                                    instance,
                                    from,
                                    to,
                                })
                                .unwrap(),
                            }
                        }
                        SupervisorEvent::Log {
                            worker,
                            instance,
                            stream,
                            line,
                        } => {
                            if !topics.iter().any(|t| t == "log") {
                                continue;
                            }
                            if let Some(f) = &worker_filter {
                                if f != &worker {
                                    continue;
                                }
                            }
                            Frame::Event {
                                topic: "log".into(),
                                payload: serde_json::to_value(LogEvent {
                                    worker,
                                    instance,
                                    stream,
                                    line,
                                })
                                .unwrap(),
                            }
                        }
                        SupervisorEvent::Metric {
                            worker,
                            instance,
                            ts,
                            cpu,
                            mem,
                        } => {
                            if !topics.iter().any(|t| t == "metric") {
                                continue;
                            }
                            if let Some(f) = &worker_filter {
                                if f != &worker {
                                    continue;
                                }
                            }
                            Frame::Event {
                                topic: "metric".into(),
                                payload: serde_json::to_value(servicio_ipc::types::MetricEvent {
                                    worker,
                                    instance,
                                    ts,
                                    cpu,
                                    mem,
                                })
                                .unwrap(),
                            }
                        }
                    };
                    if write_frame_locked(&wr, &frame).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    let frame = Frame::Event {
                        topic: "lagged".into(),
                        payload: json!({"dropped": n}),
                    };
                    if write_frame_locked(&wr, &frame).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

async fn write_frame_locked(
    wr: &Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
    frame: &Frame,
) -> std::io::Result<()> {
    let mut guard = wr.lock().await;
    guard
        .write_all(format!("{}\n", frame.to_line()).as_bytes())
        .await
}

/// Method dispatch for authenticated connections. Extended in Task 6/7.
async fn dispatch(daemon: &Arc<Daemon>, id: u64, method: &str, params: serde_json::Value) -> Frame {
    match method {
        "ping" => Frame::ok(id, json!({"pong": true})),
        "daemon_info" => {
            let mgr = daemon.manager.lock().await;
            let status = mgr.status();
            let worker_count = status.len() as u32;
            let running_count = status
                .iter()
                .filter(|w| {
                    w.instances
                        .iter()
                        .any(|i| matches!(i.state, servicio_core::state::InstanceState::Running))
                })
                .count() as u32;
            Frame::ok(
                id,
                json!({
                    "version": daemon.version,
                    "build": daemon.build,
                    "uptime_secs": daemon.started.elapsed().as_secs(),
                    "worker_count": worker_count,
                    "running_count": running_count,
                }),
            )
        }
        "list_workers" => {
            // DB holds the worker definitions (source of truth). Overlay live
            // instance status from the Manager where a worker is running.
            let specs = {
                let db = daemon.db.lock().await;
                db.list_workers()
            };
            let specs = match specs {
                Ok(s) => s,
                Err(e) => return Frame::err(id, "db_error", &e.to_string()),
            };
            let mut live: std::collections::HashMap<String, Vec<IpcInstanceStatus>> = {
                let mgr = daemon.manager.lock().await;
                mgr.status()
                    .into_iter()
                    .map(|w| {
                        let instances = w
                            .instances
                            .into_iter()
                            .map(|i| IpcInstanceStatus {
                                index: i.index,
                                state: i.state,
                                restart_count: i.restart_count,
                                pid: i.pid,
                            })
                            .collect();
                        (w.name, instances)
                    })
                    .collect()
            };
            let list: Vec<WorkerStatus> = specs
                .into_iter()
                .map(|spec| WorkerStatus {
                    instances: live.remove(&spec.name).unwrap_or_default(),
                    name: spec.name,
                    run_mode: spec.run_mode,
                    group: spec.group,
                    tags: spec.tags,
                    display_name: spec.display_name,
                })
                .collect();
            match serde_json::to_value(list) {
                Ok(v) => Frame::ok(id, v),
                Err(e) => Frame::err(id, "internal", &e.to_string()),
            }
        }
        "add_worker" => {
            let spec: Result<WorkerSpec, _> = serde_json::from_value(
                params
                    .get("spec")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            );
            match spec {
                Ok(spec) => {
                    let name = spec.name.clone();
                    let db = daemon.db.lock().await;
                    match db.upsert_worker(&spec) {
                        Ok(()) => {
                            tracing::info!("add_worker '{name}'");
                            Frame::ok(id, json!({"name": name}))
                        }
                        Err(e) => {
                            tracing::warn!("add_worker '{name}' failed: {e}");
                            Frame::err(id, "db_error", &e.to_string())
                        }
                    }
                }
                Err(e) => Frame::err(id, "bad_params", &e.to_string()),
            }
        }
        "remove_worker" => {
            let name = params
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            {
                let mut mgr = daemon.manager.lock().await;
                mgr.stop_worker(&name).await;
            }
            let db = daemon.db.lock().await;
            match db.remove_worker(&name) {
                Ok(removed) => {
                    tracing::info!("remove_worker '{name}' (removed={removed})");
                    Frame::ok(id, json!({"removed": removed}))
                }
                Err(e) => {
                    tracing::warn!("remove_worker '{name}' failed: {e}");
                    Frame::err(id, "db_error", &e.to_string())
                }
            }
        }
        "start_worker" => {
            let name = params
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let db = daemon.db.lock().await;
            let spec = db.get_worker(&name);
            drop(db);
            match spec {
                Ok(Some(spec)) => {
                    let mut mgr = daemon.manager.lock().await;
                    mgr.start_worker(spec).await;
                    tracing::info!("start_worker '{name}'");
                    Frame::ok(id, json!({"started": true}))
                }
                Ok(None) => {
                    tracing::warn!("start_worker '{name}' not found");
                    Frame::err(id, "not_found", &format!("no worker '{name}'"))
                }
                Err(e) => {
                    tracing::warn!("start_worker '{name}' failed: {e}");
                    Frame::err(id, "db_error", &e.to_string())
                }
            }
        }
        "stop_worker" => {
            let name = params
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let mut mgr = daemon.manager.lock().await;
            let stopped = mgr.stop_worker(&name).await;
            tracing::info!("stop_worker '{name}' (stopped={stopped})");
            Frame::ok(id, json!({"stopped": stopped}))
        }
        "metrics" => {
            let name = params
                .get("worker")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let since = params
                .get("since_secs")
                .and_then(|n| n.as_u64())
                .unwrap_or(0);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let floor = if since == 0 {
                0
            } else {
                now.saturating_sub(since)
            };
            let db = daemon.db.lock().await;
            match db.query_metrics(&name, floor) {
                Ok(rows) => {
                    let series: Vec<servicio_ipc::types::MetricSeries> = rows
                        .into_iter()
                        .map(|(instance, pts)| servicio_ipc::types::MetricSeries {
                            instance,
                            points: pts
                                .into_iter()
                                .map(|(ts, cpu, mem)| servicio_ipc::types::MetricPoint {
                                    ts,
                                    cpu,
                                    mem,
                                })
                                .collect(),
                        })
                        .collect();
                    match serde_json::to_value(series) {
                        Ok(v) => Frame::ok(id, v),
                        Err(e) => Frame::err(id, "internal", &e.to_string()),
                    }
                }
                Err(e) => Frame::err(id, "db_error", &e.to_string()),
            }
        }
        "detect_workers" => {
            let path = params.get("path").and_then(|p| p.as_str()).unwrap_or("");
            let suggestions = servicio_detect::detect_all(std::path::Path::new(path));
            match serde_json::to_value(suggestions) {
                Ok(v) => Frame::ok(id, v),
                Err(e) => Frame::err(id, "internal", &e.to_string()),
            }
        }
        "daemon_log" => {
            let lines = params.get("lines").and_then(|n| n.as_u64()).unwrap_or(200) as usize;
            let body = std::fs::read_to_string(&daemon.log_path).unwrap_or_default();
            let tail: Vec<&str> = body.lines().rev().take(lines).collect();
            let text: String = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
            Frame::ok(id, serde_json::json!({ "log": text }))
        }
        "get_worker" => {
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let db = daemon.db.lock().await;
            match db.get_worker(name) {
                Ok(Some(spec)) => match serde_json::to_value(spec) {
                    Ok(v) => Frame::ok(id, v),
                    Err(e) => Frame::err(id, "internal", &e.to_string()),
                },
                Ok(None) => Frame::err(id, "not_found", &format!("no worker '{name}'")),
                Err(e) => Frame::err(id, "db_error", &e.to_string()),
            }
        }
        "shutdown" => {
            daemon.shutdown.notify_one();
            Frame::ok(id, serde_json::json!({"shutting_down": true}))
        }
        "start_group" => {
            let group = params.get("group").and_then(|g| g.as_str()).unwrap_or("");
            // Workers whose spec.group matches (treat "Ungrouped"/empty as None).
            let want_none = group.is_empty() || group == "Ungrouped";
            let specs: Vec<servicio_core::worker::WorkerSpec> = {
                let db = daemon.db.lock().await;
                match db.list_workers() {
                    Ok(all) => all
                        .into_iter()
                        .filter(|s| match &s.group {
                            Some(g) => g == group,
                            None => want_none,
                        })
                        .collect(),
                    Err(e) => return Frame::err(id, "db_error", &e.to_string()),
                }
            };
            let mut started = 0u32;
            {
                let mut mgr = daemon.manager.lock().await;
                for spec in specs {
                    mgr.start_worker(spec).await;
                    started += 1;
                }
            }
            tracing::info!("start_group '{group}' ({started})");
            Frame::ok(id, json!({"started": started}))
        }
        "stop_group" => {
            let group = params.get("group").and_then(|g| g.as_str()).unwrap_or("");
            let want_none = group.is_empty() || group == "Ungrouped";
            let names: Vec<String> = {
                let db = daemon.db.lock().await;
                match db.list_workers() {
                    Ok(all) => all
                        .into_iter()
                        .filter(|s| match &s.group {
                            Some(g) => g == group,
                            None => want_none,
                        })
                        .map(|s| s.name)
                        .collect(),
                    Err(e) => return Frame::err(id, "db_error", &e.to_string()),
                }
            };
            let mut stopped = 0u32;
            {
                let mut mgr = daemon.manager.lock().await;
                for name in names {
                    if mgr.stop_worker(&name).await {
                        stopped += 1;
                    }
                }
            }
            tracing::info!("stop_group '{group}' ({stopped})");
            Frame::ok(id, json!({"stopped": stopped}))
        }
        "export_workers" => {
            let db = daemon.db.lock().await;
            match db.list_workers() {
                Ok(specs) => match serde_json::to_value(specs) {
                    Ok(v) => Frame::ok(id, json!({ "workers": v })),
                    Err(e) => Frame::err(id, "internal", &e.to_string()),
                },
                Err(e) => Frame::err(id, "db_error", &e.to_string()),
            }
        }
        "import_workers" => {
            let specs: Result<Vec<servicio_core::worker::WorkerSpec>, _> = serde_json::from_value(
                params
                    .get("workers")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            );
            match specs {
                Ok(specs) => {
                    let db = daemon.db.lock().await;
                    let mut n = 0u32;
                    for s in &specs {
                        if db.upsert_worker(s).is_ok() {
                            n += 1;
                        }
                    }
                    tracing::info!("import_workers ({n})");
                    Frame::ok(id, json!({ "imported": n }))
                }
                Err(e) => Frame::err(id, "bad_params", &e.to_string()),
            }
        }
        other => Frame::err(id, "unknown_method", &format!("no such method: {other}")),
    }
}

/// Read one '\n'-terminated line into `buf` (without the newline), capping total
/// bytes at `max`. Returns Ok(None) on clean EOF, Err on a too-long line.
async fn read_line_capped(
    reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
    buf: &mut Vec<u8>,
    max: usize,
) -> std::io::Result<Option<()>> {
    buf.clear();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return if buf.is_empty() {
                Ok(None)
            } else {
                Ok(Some(()))
            };
        }
        if let Some(pos) = available.iter().position(|&b| b == b'\n') {
            buf.extend_from_slice(&available[..pos]);
            reader.consume(pos + 1);
            return Ok(Some(()));
        }
        buf.extend_from_slice(available);
        let n = available.len();
        reader.consume(n);
        if buf.len() > max {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "line too long",
            ));
        }
    }
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

#[cfg(unix)]
fn set_dir_private(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_dir_private(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

fn to_io<E: std::fmt::Display>(e: E) -> std::io::Error {
    std::io::Error::other(e.to_string())
}
