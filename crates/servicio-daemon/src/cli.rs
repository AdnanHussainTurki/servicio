use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "servicio-daemon",
    about = "Servicio supervisor (phase 1 test CLI)"
)]
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
        // Pass each worker arg as a separate token, e.g.:
        //   --args -c "while true; do echo tick; sleep 1; done"
        // allow_hyphen_values lets values like `-c` through; num_args collects them all.
        #[arg(long, num_args = 0.., allow_hyphen_values = true)]
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
    /// Run the daemon: bind the socket and supervise workers until terminated.
    Serve {
        /// Base dir for socket/token/lock/db (defaults to the runtime dir).
        #[arg(long)]
        base: Option<PathBuf>,
    },
    /// Install the daemon as a login service (launchd/systemd).
    InstallService {
        #[arg(long)]
        base: Option<PathBuf>,
    },
    /// Remove the installed login service.
    UninstallService,
    /// Show whether the login service is installed.
    ServiceStatus,
    /// Print this binary's build id (used by the GUI to detect a stale daemon).
    BuildId,
}
