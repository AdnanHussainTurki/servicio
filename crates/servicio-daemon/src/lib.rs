pub mod cli;
pub mod db;
pub mod lock;
pub mod paths;
pub mod sampler;
pub mod serve;
pub mod token;

use db::Db;
use servicio_core::worker::{RestartPolicy, RunMode, WorkerSpec};
use std::collections::BTreeMap;
use std::path::Path;

/// Add (or replace) a worker definition in the database.
#[allow(clippy::too_many_arguments)]
pub fn add_worker(
    db_path: &Path,
    name: &str,
    command: &str,
    args: &[String],
    working_dir: &Path,
    concurrency: u32,
    autostart: bool,
) -> rusqlite::Result<()> {
    let spec = WorkerSpec {
        name: name.to_string(),
        command: command.to_string(),
        args: args.to_vec(),
        working_dir: working_dir.to_path_buf(),
        env: BTreeMap::new(),
        run_mode: RunMode::Daemon { concurrency },
        restart: RestartPolicy::default(),
        autostart,
        enabled: true,
    };
    let db = Db::open(db_path)?;
    db.upsert_worker(&spec)
}

/// The reconcile step the daemon runs on startup: which workers should be running?
pub fn reconcile_specs(db_path: &Path) -> rusqlite::Result<Vec<WorkerSpec>> {
    let db = Db::open(db_path)?;
    db.autostart_workers()
}
