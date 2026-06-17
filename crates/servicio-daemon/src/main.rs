use clap::Parser;
use servicio_core::manager::Manager;
use servicio_core::process::TokioProcess;
use servicio_daemon_lib::cli::{Cli, Command};
use servicio_daemon_lib::{add_worker, db::Db, reconcile_specs};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Add { name, command, args, working_dir, concurrency, autostart } => {
            add_worker(&cli.db, &name, &command, &args, &working_dir, concurrency, autostart)?;
            println!("added worker '{name}'");
        }
        Command::List => {
            let db = Db::open(&cli.db)?;
            for w in db.list_workers()? {
                println!("{}  cmd={} {:?}  mode={:?}  autostart={}", w.name, w.command, w.args, w.run_mode, w.autostart);
            }
        }
        Command::Run => {
            let specs = reconcile_specs(&cli.db)?;
            let log_dir = std::env::temp_dir().join("servicio-logs");
            let mut mgr = Manager::new(Arc::new(TokioProcess), log_dir);
            for spec in specs {
                println!("starting '{}'", spec.name);
                mgr.start_worker(spec).await;
            }
            println!("supervising; press Ctrl-C to stop");
            tokio::signal::ctrl_c().await?;
            mgr.stop_all().await;
            println!("stopped");
        }
    }
    Ok(())
}
