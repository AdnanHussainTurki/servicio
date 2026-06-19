use crate::sidecar::{ensure_daemon, run_daemon_subcommand};
use crate::state::AppState;
use serde::Serialize;
use servicio_core::worker::WorkerSpec;
use servicio_ipc::types::WorkerStatus;
use std::time::Duration;

pub fn service_status() -> Result<serde_json::Value, String> {
    let out = run_daemon_subcommand(&["service-status"]).map_err(|e| e.to_string())?;
    serde_json::from_str(out.trim()).map_err(|e| format!("parse service-status: {e} (got: {out})"))
}

pub fn install_service() -> Result<(), String> {
    run_daemon_subcommand(&["install-service"])
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn uninstall_service() -> Result<(), String> {
    run_daemon_subcommand(&["uninstall-service"])
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub async fn list_workers(state: &AppState) -> Result<Vec<WorkerStatus>, String> {
    let mut client = state.client.lock().await;
    client.list_workers().await.map_err(|e| e.to_string())
}

pub async fn add_worker(state: &AppState, spec: WorkerSpec) -> Result<(), String> {
    let mut client = state.client.lock().await;
    client.add_worker(&spec).await.map_err(|e| e.to_string())
}

pub async fn get_worker(state: &AppState, name: &str) -> Result<serde_json::Value, String> {
    let mut c = state.client.lock().await;
    c.get_worker(name).await.map_err(|e| e.to_string())
}

pub async fn start_worker(state: &AppState, name: &str) -> Result<(), String> {
    let mut client = state.client.lock().await;
    client.start_worker(name).await.map_err(|e| e.to_string())
}

pub async fn stop_worker(state: &AppState, name: &str) -> Result<(), String> {
    let mut client = state.client.lock().await;
    client.stop_worker(name).await.map_err(|e| e.to_string())
}

pub async fn restart_worker(state: &AppState, name: &str) -> Result<(), String> {
    {
        let mut client = state.client.lock().await;
        client.stop_worker(name).await.map_err(|e| e.to_string())?;
    }
    let mut client = state.client.lock().await;
    client.start_worker(name).await.map_err(|e| e.to_string())
}

pub async fn remove_worker(state: &AppState, name: &str) -> Result<(), String> {
    let mut c = state.client.lock().await;
    c.remove_worker(name)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub async fn export_workers_to(state: &AppState, path: &str) -> Result<u32, String> {
    let v = {
        let mut c = state.client.lock().await;
        c.export_workers().await.map_err(|e| e.to_string())?
    };
    let arr = v
        .get("workers")
        .cloned()
        .unwrap_or(serde_json::Value::Array(vec![]));
    let count = arr.as_array().map(|a| a.len()).unwrap_or(0) as u32;
    let pretty = serde_json::to_string_pretty(&arr).map_err(|e| e.to_string())?;
    std::fs::write(path, pretty).map_err(|e| e.to_string())?;
    Ok(count)
}

pub async fn import_workers_from(state: &AppState, path: &str) -> Result<u32, String> {
    let body = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let workers: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("invalid config: {e}"))?;
    let mut c = state.client.lock().await;
    let res = c.import_workers(workers).await.map_err(|e| e.to_string())?;
    Ok(res.get("imported").and_then(|n| n.as_u64()).unwrap_or(0) as u32)
}

pub async fn start_group(state: &AppState, group: &str) -> Result<serde_json::Value, String> {
    let mut c = state.client.lock().await;
    c.start_group(group).await.map_err(|e| e.to_string())
}

pub async fn stop_group(state: &AppState, group: &str) -> Result<serde_json::Value, String> {
    let mut c = state.client.lock().await;
    c.stop_group(group).await.map_err(|e| e.to_string())
}

fn service_installed() -> bool {
    service_status()
        .ok()
        .and_then(|v| v.get("installed").and_then(|b| b.as_bool()))
        .unwrap_or(false)
}

/// Full stop: gracefully stop every worker, then stop the daemon itself. When
/// the login service is installed we unload it (launchd KeepAlive would
/// otherwise respawn the daemon instantly); otherwise we ask the GUI-spawned
/// daemon to exit.
pub async fn stop_daemon(state: &AppState) -> Result<(), String> {
    // 1. Gracefully stop all workers (best-effort per worker).
    let workers = {
        let mut c = state.client.lock().await;
        c.list_workers().await.map_err(|e| e.to_string())?
    };
    for w in &workers {
        let mut c = state.client.lock().await;
        let _ = c.stop_worker(&w.name).await;
    }
    // 2. Stop the daemon process.
    if service_installed() {
        run_daemon_subcommand(&["stop-service"]).map_err(|e| e.to_string())?;
    }
    // Also ask it to exit directly — covers the sidecar (no service) case and
    // any launchd respawn race.
    {
        let mut c = state.client.lock().await;
        let _ = c.shutdown().await;
    }
    Ok(())
}

/// Start the daemon again and reconnect the client. Loads the login service if
/// installed, else spawns the bundled sidecar.
pub async fn start_daemon(state: &AppState, daemon_program: &str) -> Result<(), String> {
    if service_installed() {
        run_daemon_subcommand(&["start-service"]).map_err(|e| e.to_string())?;
    } else {
        ensure_daemon(&state.base, daemon_program)
            .await
            .map_err(|e| e.to_string())?;
    }
    // Wait for the new daemon to bind, then swap in a fresh connection.
    for _ in 0..50 {
        if state.reconnect().await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err("daemon did not become ready".into())
}

#[derive(Serialize)]
pub struct DaemonStatus {
    pub connected: bool,
    pub version: String,
    pub uptime_secs: u64,
    pub worker_count: u32,
    pub running_count: u32,
}

pub async fn detect_workers(state: &AppState, path: &str) -> Result<serde_json::Value, String> {
    let mut client = state.client.lock().await;
    client.detect(path).await.map_err(|e| e.to_string())
}

pub async fn metrics(
    state: &AppState,
    worker: &str,
    since_secs: u64,
) -> Result<serde_json::Value, String> {
    let mut client = state.client.lock().await;
    client
        .metrics(worker, since_secs)
        .await
        .map_err(|e| e.to_string())
}

pub async fn daemon_log(state: &AppState, lines: u64) -> Result<serde_json::Value, String> {
    let mut client = state.client.lock().await;
    client.daemon_log(lines).await.map_err(|e| e.to_string())
}

pub async fn daemon_status(state: &AppState) -> Result<DaemonStatus, String> {
    let mut client = state.client.lock().await;
    match client.daemon_info().await {
        Ok(v) => Ok(DaemonStatus {
            connected: true,
            version: v
                .get("version")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            uptime_secs: v.get("uptime_secs").and_then(|x| x.as_u64()).unwrap_or(0),
            worker_count: v.get("worker_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            running_count: v.get("running_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
        }),
        Err(e) => Err(e.to_string()),
    }
}
