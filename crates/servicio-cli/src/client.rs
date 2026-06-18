//! Thin async client for the servicio daemon.

use anyhow::{anyhow, Result};
use servicio_ipc::types::WorkerStatus;
use servicio_ipc::Frame;
use serde_json::{json, Value};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;

pub struct Client {
    wr: OwnedWriteHalf,
    lines: Lines<BufReader<OwnedReadHalf>>,
    next_id: u64,
}

impl Client {
    /// Connect and perform the `hello` handshake.
    pub async fn connect(socket: &Path, token: &str) -> Result<Self> {
        let stream = UnixStream::connect(socket).await?;
        let (rd, wr) = stream.into_split();
        let lines = BufReader::new(rd).lines();
        let mut c = Client { wr, lines, next_id: 1 };
        let _ = c.request("hello", json!({ "token": token })).await?;
        Ok(c)
    }

    /// Send a request and await its matching response (skips interleaved events).
    pub async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let frame = Frame::Request { id, method: method.into(), params };
        self.wr.write_all(format!("{}\n", frame.to_line()).as_bytes()).await?;
        loop {
            let line = self
                .lines
                .next_line()
                .await?
                .ok_or_else(|| anyhow!("connection closed"))?;
            match Frame::from_line(&line)? {
                Frame::Response { id: rid, result, error } if rid == id => {
                    if let Some(e) = error {
                        return Err(anyhow!("{}: {}", e.code, e.message));
                    }
                    return Ok(result.unwrap_or(Value::Null));
                }
                _ => continue,
            }
        }
    }

    pub async fn ping(&mut self) -> Result<()> {
        self.request("ping", json!({})).await.map(|_| ())
    }

    pub async fn add_worker(&mut self, spec: &servicio_core::worker::WorkerSpec) -> Result<()> {
        self.request("add_worker", json!({ "spec": spec })).await.map(|_| ())
    }

    pub async fn list_workers(&mut self) -> Result<Vec<WorkerStatus>> {
        let v = self.request("list_workers", json!({})).await?;
        Ok(serde_json::from_value(v)?)
    }

    pub async fn start_worker(&mut self, name: &str) -> Result<()> {
        self.request("start_worker", json!({ "name": name })).await.map(|_| ())
    }

    pub async fn stop_worker(&mut self, name: &str) -> Result<()> {
        self.request("stop_worker", json!({ "name": name })).await.map(|_| ())
    }

    pub async fn get_worker(&mut self, name: &str) -> Result<serde_json::Value> {
        self.request("get_worker", json!({"name": name})).await
    }

    pub async fn daemon_info(&mut self) -> Result<Value> {
        self.request("daemon_info", json!({})).await
    }

    pub async fn shutdown(&mut self) -> Result<serde_json::Value> {
        self.request("shutdown", json!({})).await
    }

    pub async fn metrics(&mut self, worker: &str, since_secs: u64) -> Result<serde_json::Value> {
        self.request("metrics", json!({ "worker": worker, "since_secs": since_secs })).await
    }

    pub async fn detect(&mut self, path: &str) -> Result<serde_json::Value> {
        self.request("detect_workers", json!({ "path": path })).await
    }

    pub async fn daemon_log(&mut self, lines: u64) -> Result<serde_json::Value> {
        self.request("daemon_log", json!({ "lines": lines })).await
    }

    /// Send a subscribe request, consume the ack, then yield raw event lines.
    pub async fn subscribe(
        mut self,
        topics: &[&str],
        worker: Option<&str>,
    ) -> Result<Lines<BufReader<OwnedReadHalf>>> {
        let id = self.next_id;
        let params = json!({ "topics": topics, "worker": worker });
        let frame = Frame::Request { id, method: "subscribe".into(), params };
        self.wr.write_all(format!("{}\n", frame.to_line()).as_bytes()).await?;
        loop {
            let line = self.lines.next_line().await?.ok_or_else(|| anyhow!("closed"))?;
            if let Frame::Response { id: rid, .. } = Frame::from_line(&line)? {
                if rid == id { break; }
            }
        }
        Ok(self.lines)
    }
}
