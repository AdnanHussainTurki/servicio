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

pub fn socket_path(base: &std::path::Path) -> PathBuf { base.join("daemon.sock") }
pub fn token_path(base: &std::path::Path) -> PathBuf { base.join("token") }

pub async fn ensure_daemon(base: &std::path::Path, daemon_program: &str) -> Result<String> {
    std::fs::create_dir_all(base).ok();
    if let Ok(token) = read_token(base) {
        if Client::connect(&socket_path(base), &token).await.is_ok() {
            return Ok(token);
        }
    }
    Command::new(daemon_program)
        .arg("serve").arg("--base").arg(base)
        .spawn()
        .map_err(|e| anyhow!("spawn daemon: {e}"))?;
    for _ in 0..50 {
        if let Ok(token) = read_token(base) {
            if Client::connect(&socket_path(base), &token).await.is_ok() {
                return Ok(token);
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(anyhow!("daemon did not become ready"))
}

fn read_token(base: &std::path::Path) -> Result<String> {
    Ok(std::fs::read_to_string(token_path(base))?.trim().to_string())
}
