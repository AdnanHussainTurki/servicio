use anyhow::Result;
use servicio_cli_lib::Client;
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

pub struct AppState {
    pub client: Mutex<Client>,
    pub base: PathBuf,
    pub token: String,
}

impl AppState {
    pub async fn connect(base: &Path, token: &str) -> Result<Self> {
        let client = Client::connect(base, token).await?;
        Ok(Self {
            client: Mutex::new(client),
            base: base.to_path_buf(),
            token: token.to_string(),
        })
    }

    /// Re-read the token and replace the client connection (used after the
    /// daemon is stopped and started again, which mints a fresh socket).
    pub async fn reconnect(&self) -> Result<()> {
        let token = std::fs::read_to_string(self.base.join("token"))?
            .trim()
            .to_string();
        let client = Client::connect(&self.base, &token).await?;
        *self.client.lock().await = client;
        Ok(())
    }
}
