use anyhow::{anyhow, Result};
use servicio_cli_lib::Client;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;

pub fn default_base() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(dir).join("servicio")
    } else {
        std::env::temp_dir().join("servicio")
    }
}

pub fn socket_path(base: &std::path::Path) -> PathBuf {
    base.join("daemon.sock")
}
pub fn token_path(base: &std::path::Path) -> PathBuf {
    base.join("token")
}

/// Resolve the bundled `servicio-daemon` binary path, falling back to PATH.
pub fn daemon_program() -> String {
    // Tauri drops the sidecar next to the main exe, named with the platform's
    // executable extension (`servicio-daemon.exe` on Windows).
    let name = if cfg!(windows) {
        "servicio-daemon.exe"
    } else {
        "servicio-daemon"
    };
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(name)))
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "servicio-daemon".to_string())
}

/// Run a daemon subcommand, capturing stdout. Used for service-status/install/uninstall.
pub fn run_daemon_subcommand(args: &[&str]) -> std::io::Result<String> {
    let out = std::process::Command::new(daemon_program())
        .args(args)
        .output()?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The build id of the daemon binary the GUI would spawn (its bundled sidecar).
pub fn bundled_build_id() -> Option<String> {
    let out = std::process::Command::new(daemon_program())
        .arg("build-id")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

pub async fn ensure_daemon(base: &std::path::Path, daemon_program: &str) -> Result<String> {
    std::fs::create_dir_all(base).ok();

    // Is a daemon already running? If so, check it isn't stale.
    if let Ok(token) = read_token(base) {
        if let Ok(mut client) = Client::connect(base, &token).await {
            let running_build = client
                .daemon_info()
                .await
                .ok()
                .and_then(|v| v.get("build").and_then(|b| b.as_str().map(String::from)));
            let bundled = bundled_build_id();
            // Stale only when we can determine the bundled build AND it differs
            // from the running daemon's build (a missing `build` field counts as
            // a mismatch, so pre-feature daemons get replaced).
            let stale = matches!(&bundled, Some(b) if running_build.as_deref() != Some(b.as_str()));
            if !stale {
                return Ok(token); // current (or build undeterminable) — use it
            }
            // Stale daemon: ask it to exit, then wait for it to release the endpoint.
            let _ = client.shutdown().await;
            drop(client);
            // Unix: watch the socket file disappear. Windows: the named pipe has no
            // filesystem presence, so just give the old daemon a moment to drop it.
            #[cfg(unix)]
            for _ in 0..50 {
                if !socket_path(base).exists() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            #[cfg(windows)]
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }

    // Spawn the bundled daemon and wait for it to become ready.
    Command::new(daemon_program)
        .arg("serve")
        .arg("--base")
        .arg(base)
        .spawn()
        .map_err(|e| anyhow!("spawn daemon: {e}"))?;
    for _ in 0..50 {
        if let Ok(token) = read_token(base) {
            if Client::connect(base, &token).await.is_ok() {
                return Ok(token);
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(anyhow!("daemon did not become ready"))
}

fn read_token(base: &std::path::Path) -> Result<String> {
    Ok(std::fs::read_to_string(token_path(base))?
        .trim()
        .to_string())
}
