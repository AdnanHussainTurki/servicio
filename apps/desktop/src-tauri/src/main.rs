#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use servicio_app::bridge::{self, DaemonStatus};
use servicio_app::events::run_event_pump;
use servicio_app::sidecar::{daemon_program, default_base, ensure_daemon, socket_path};
use servicio_app::state::AppState;
use servicio_core::worker::WorkerSpec;
use servicio_ipc::types::WorkerStatus;
use tauri::{Emitter, Manager};

#[tauri::command]
async fn daemon_status(state: tauri::State<'_, AppState>) -> Result<DaemonStatus, String> {
    bridge::daemon_status(&state).await
}

#[tauri::command]
async fn list_workers(state: tauri::State<'_, AppState>) -> Result<Vec<WorkerStatus>, String> {
    bridge::list_workers(&state).await
}

#[tauri::command]
async fn add_worker(state: tauri::State<'_, AppState>, spec: WorkerSpec) -> Result<(), String> {
    bridge::add_worker(&state, spec).await
}

#[tauri::command]
async fn start_worker(state: tauri::State<'_, AppState>, name: String) -> Result<(), String> {
    bridge::start_worker(&state, &name).await
}

#[tauri::command]
async fn stop_worker(state: tauri::State<'_, AppState>, name: String) -> Result<(), String> {
    bridge::stop_worker(&state, &name).await
}

#[tauri::command]
async fn restart_worker(state: tauri::State<'_, AppState>, name: String) -> Result<(), String> {
    bridge::restart_worker(&state, &name).await
}

#[tauri::command]
async fn detect_workers(state: tauri::State<'_, AppState>, path: String) -> Result<serde_json::Value, String> {
    bridge::detect_workers(&state, &path).await
}

#[tauri::command]
async fn metrics(state: tauri::State<'_, AppState>, worker: String, since_secs: u64) -> Result<serde_json::Value, String> {
    bridge::metrics(&state, &worker, since_secs).await
}

#[tauri::command]
fn service_status() -> Result<serde_json::Value, String> {
    bridge::service_status()
}

#[tauri::command]
fn install_service() -> Result<(), String> {
    bridge::install_service()
}

#[tauri::command]
fn uninstall_service() -> Result<(), String> {
    bridge::uninstall_service()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let base = default_base();
                let daemon_program = daemon_program();
                let token = {
                    let mut attempt = 0;
                    loop {
                        attempt += 1;
                        match ensure_daemon(&base, daemon_program.as_str()).await {
                            Ok(t) => break t,
                            Err(e) if attempt < 3 => {
                                eprintln!("daemon not ready (attempt {attempt}/3): {e}");
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            }
                            Err(e) => {
                                eprintln!("daemon not ready: {e}");
                                return;
                            }
                        }
                    }
                };
                let socket = socket_path(&base);
                if let Ok(state) = AppState::connect(&socket, &token).await {
                    handle.manage(state);
                }
                let emit_handle = handle.clone();
                run_event_pump(socket, token, move |payload| {
                    let _ = emit_handle.emit("worker-event", payload);
                })
                .await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            daemon_status,
            list_workers,
            add_worker,
            start_worker,
            stop_worker,
            restart_worker,
            detect_workers,
            metrics,
            service_status,
            install_service,
            uninstall_service
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
