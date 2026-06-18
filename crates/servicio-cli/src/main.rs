use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use servicio_cli_lib::Client;
use servicio_ipc::Frame;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "servicio", about = "Control the servicio daemon")]
struct Cli {
    /// Base dir where the daemon's socket + token live.
    #[arg(long)]
    base: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List workers and their status.
    Ps,
    /// Daemon info.
    Info,
    /// Start a worker.
    Start { name: String },
    /// Stop a worker.
    Stop { name: String },
    /// Stream logs for a worker (follow until Ctrl-C).
    Logs { name: String },
    /// Show recent metrics (cpu/mem) for a worker.
    Metrics { name: String },
    /// Scan a folder and suggest workers (autodetect).
    Detect { path: String },
    /// Show the tail of the daemon's own log.
    DaemonLog {
        /// Number of trailing log lines to show.
        #[arg(long, default_value_t = 200)]
        lines: u64,
    },
}

fn base_dir(arg: Option<PathBuf>) -> PathBuf {
    arg.unwrap_or_else(|| {
        if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
            PathBuf::from(dir).join("servicio")
        } else {
            std::env::temp_dir().join("servicio")
        }
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let base = base_dir(cli.base);
    let socket = base.join("daemon.sock");
    let token = std::fs::read_to_string(base.join("token"))
        .context("reading token (is the daemon running?)")?
        .trim()
        .to_string();

    let mut client = Client::connect(&socket, &token).await.context("connecting to daemon")?;

    match cli.command {
        Command::Ps => {
            let workers = client.list_workers().await?;
            println!("{:<20} {:<22} {:<10} RESTARTS", "NAME", "MODE", "STATE");
            for w in workers {
                let state = w.instances.first().map(|i| format!("{:?}", i.state)).unwrap_or_else(|| "-".into());
                let restarts: u32 = w.instances.iter().map(|i| i.restart_count).sum();
                println!("{:<20} {:<22} {:<10} {}", w.name, format!("{:?}", w.run_mode), state, restarts);
            }
        }
        Command::Info => {
            let info = client.daemon_info().await?;
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        Command::Start { name } => {
            client.start_worker(&name).await?;
            println!("started '{name}'");
        }
        Command::Stop { name } => {
            client.stop_worker(&name).await?;
            println!("stopped '{name}'");
        }
        Command::Metrics { name } => {
            let v = client.metrics(&name, 900).await?;
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        Command::Detect { path } => {
            let v = client.detect(&path).await?;
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        Command::DaemonLog { lines } => {
            let v = client.daemon_log(lines).await?;
            println!("{}", v.get("log").and_then(|l| l.as_str()).unwrap_or(""));
        }
        Command::Logs { name } => {
            let mut lines = client.subscribe(&["log"], Some(&name)).await?;
            println!("following logs for '{name}' (Ctrl-C to stop)");
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(Frame::Event { topic, payload }) = Frame::from_line(&line) {
                    if topic == "log" {
                        let l = payload.get("line").and_then(|v| v.as_str()).unwrap_or("");
                        let stream = payload.get("stream").and_then(|v| v.as_str()).unwrap_or("");
                        println!("[{stream}] {l}");
                    }
                }
            }
        }
    }
    Ok(())
}
