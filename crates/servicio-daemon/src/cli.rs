use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "servicio-daemon", about = "Servicio supervisor (phase 1 test CLI)")]
pub struct Cli {
    /// Path to the SQLite database.
    #[arg(long, default_value = "servicio.db")]
    pub db: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Add or replace a worker definition.
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        command: String,
        #[arg(long, value_delimiter = ' ')]
        args: Vec<String>,
        #[arg(long, default_value = ".")]
        working_dir: PathBuf,
        #[arg(long, default_value_t = 1)]
        concurrency: u32,
        #[arg(long, default_value_t = true)]
        autostart: bool,
    },
    /// List stored workers.
    List,
    /// Load autostart workers and supervise them until Ctrl-C.
    Run,
}
