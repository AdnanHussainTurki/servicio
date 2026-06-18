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
}
