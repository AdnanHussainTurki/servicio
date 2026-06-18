//! Platform IPC transport for the local daemon <-> client channel.
//!
//! Unix: a filesystem Unix-domain socket under the base dir (`daemon.sock`).
//! Windows: a named pipe (`\\.\pipe\servicio-<hash-of-base>`), one instance per
//! connection. The hash of the base dir keeps independent daemon instances (and
//! test instances) on distinct pipes.
//!
//! Both backends expose the same surface: an `endpoint(base)` name, a `Listener`
//! that `accept()`s server streams, a `connect(endpoint)` for clients, and
//! split-half type aliases so `serve.rs`/`client.rs` stay platform-neutral.

use std::path::Path;

/// The transport endpoint name for a daemon rooted at `base`.
///
/// Unix returns the socket file path; Windows returns the named-pipe name.
#[cfg(unix)]
pub fn endpoint(base: &Path) -> String {
    base.join("daemon.sock").to_string_lossy().into_owned()
}

#[cfg(windows)]
pub fn endpoint(base: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    base.hash(&mut h);
    format!(r"\\.\pipe\servicio-{:016x}", h.finish())
}

// ---------------------------------------------------------------------------
// Server side
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub type ServerStream = tokio::net::UnixStream;
#[cfg(windows)]
pub type ServerStream = tokio::net::windows::named_pipe::NamedPipeServer;

#[cfg(unix)]
pub type ServerRead = tokio::net::unix::OwnedReadHalf;
#[cfg(unix)]
pub type ServerWrite = tokio::net::unix::OwnedWriteHalf;
#[cfg(windows)]
pub type ServerRead = tokio::io::ReadHalf<ServerStream>;
#[cfg(windows)]
pub type ServerWrite = tokio::io::WriteHalf<ServerStream>;

/// A bound listener that yields one server stream per accepted connection.
#[cfg(unix)]
pub struct Listener {
    inner: tokio::net::UnixListener,
}

#[cfg(unix)]
impl Listener {
    /// Bind the endpoint. The caller is responsible for removing any stale
    /// socket file and setting permissions (see `serve.rs`).
    pub fn bind(endpoint: &str) -> std::io::Result<Self> {
        Ok(Self {
            inner: tokio::net::UnixListener::bind(endpoint)?,
        })
    }

    /// Await the next inbound connection.
    pub async fn accept(&mut self) -> std::io::Result<ServerStream> {
        let (stream, _addr) = self.inner.accept().await?;
        Ok(stream)
    }
}

#[cfg(windows)]
pub struct Listener {
    name: String,
    server: Option<ServerStream>,
}

#[cfg(windows)]
impl Listener {
    /// Create the first pipe instance. Each `accept` swaps in a fresh instance
    /// so the next client can connect while the previous one is being served.
    pub fn bind(endpoint: &str) -> std::io::Result<Self> {
        use tokio::net::windows::named_pipe::ServerOptions;
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(endpoint)?;
        Ok(Self {
            name: endpoint.to_string(),
            server: Some(server),
        })
    }

    /// Await the next inbound connection, then stage the next pipe instance.
    pub async fn accept(&mut self) -> std::io::Result<ServerStream> {
        use tokio::net::windows::named_pipe::ServerOptions;
        let server = self
            .server
            .take()
            .expect("listener always holds a pending pipe instance");
        server.connect().await?;
        self.server = Some(ServerOptions::new().create(&self.name)?);
        Ok(server)
    }
}

/// Split a server stream into owned read/write halves for concurrent use.
#[cfg(unix)]
pub fn split_server(stream: ServerStream) -> (ServerRead, ServerWrite) {
    stream.into_split()
}

#[cfg(windows)]
pub fn split_server(stream: ServerStream) -> (ServerRead, ServerWrite) {
    tokio::io::split(stream)
}

// ---------------------------------------------------------------------------
// Client side
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub type ClientRead = tokio::net::unix::OwnedReadHalf;
#[cfg(unix)]
pub type ClientWrite = tokio::net::unix::OwnedWriteHalf;
#[cfg(windows)]
pub type ClientRead = tokio::io::ReadHalf<tokio::net::windows::named_pipe::NamedPipeClient>;
#[cfg(windows)]
pub type ClientWrite = tokio::io::WriteHalf<tokio::net::windows::named_pipe::NamedPipeClient>;

/// Connect to a running daemon and return owned read/write halves.
#[cfg(unix)]
pub async fn connect(endpoint: &str) -> std::io::Result<(ClientRead, ClientWrite)> {
    let stream = tokio::net::UnixStream::connect(endpoint).await?;
    Ok(stream.into_split())
}

#[cfg(windows)]
pub async fn connect(endpoint: &str) -> std::io::Result<(ClientRead, ClientWrite)> {
    use std::time::Duration;
    use tokio::net::windows::named_pipe::ClientOptions;

    // ERROR_PIPE_BUSY: all instances are busy; retry briefly until one frees.
    const ERROR_PIPE_BUSY: i32 = 231;
    let client = loop {
        match ClientOptions::new().open(endpoint) {
            Ok(c) => break c,
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(e) => return Err(e),
        }
    };
    Ok(tokio::io::split(client))
}
