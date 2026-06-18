use crate::sidecar::run_daemon_subcommand;
use crate::state::AppState;
use serde::Serialize;
use servicio_core::worker::WorkerSpec;
use servicio_ipc::types::WorkerStatus;

pub fn service_status() -> Result<serde_json::Value, String> {
    let out = run_daemon_subcommand(&["service-status"]).map_err(|e| e.to_string())?;
    serde_json::from_str(out.trim()).map_err(|e| format!("parse service-status: {e} (got: {out})"))
}

pub fn install_service() -> Result<(), String> {
    run_daemon_subcommand(&["install-service"]).map(|_| ()).map_err(|e| e.to_string())
}

pub fn uninstall_service() -> Result<(), String> {
    run_daemon_subcommand(&["uninstall-service"]).map(|_| ()).map_err(|e| e.to_string())
}

pub async fn list_workers(state: &AppState) -> Result<Vec<WorkerStatus>, String> {
    let mut client = state.client.lock().await;
    client.list_workers().await.map_err(|e| e.to_string())
}

pub async fn add_worker(state: &AppState, spec: WorkerSpec) -> Result<(), String> {
    let mut client = state.client.lock().await;
    client.add_worker(&spec).await.map_err(|e| e.to_string())
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

pub async fn metrics(state: &AppState, worker: &str, since_secs: u64) -> Result<serde_json::Value, String> {
    let mut client = state.client.lock().await;
    client.metrics(worker, since_secs).await.map_err(|e| e.to_string())
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
            version: v.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            uptime_secs: v.get("uptime_secs").and_then(|x| x.as_u64()).unwrap_or(0),
            worker_count: v.get("worker_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            running_count: v.get("running_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
        }),
        Err(e) => Err(e.to_string()),
    }
}
