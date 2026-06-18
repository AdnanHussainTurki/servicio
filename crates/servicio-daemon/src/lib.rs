pub mod cli;
pub mod db;
pub mod lock;
pub mod paths;
pub mod sampler;
pub mod serve;
pub mod service;
pub mod token;

use db::Db;
use service::ServiceSpec;
use servicio_core::worker::{RestartPolicy, RunMode, WorkerSpec};
use std::collections::BTreeMap;
use std::path::Path;

/// Build the ServiceSpec for the current daemon exe + base dir.
pub fn service_spec(base: std::path::PathBuf) -> std::io::Result<ServiceSpec> {
    let exe = std::env::current_exe()?;
    Ok(ServiceSpec { label: "com.servicio.daemon".into(), exe, base })
}

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
        group: None,
        tags: Vec::new(),
    };
    let db = Db::open(db_path)?;
    db.upsert_worker(&spec)
}

/// The reconcile step the daemon runs on startup: which workers should be running?
pub fn reconcile_specs(db_path: &Path) -> rusqlite::Result<Vec<WorkerSpec>> {
    let db = Db::open(db_path)?;
    db.autostart_workers()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn install_then_status_then_uninstall_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let spec = service_spec(std::path::PathBuf::from("/tmp/servicio")).unwrap();
        let p = service::install_to(&spec, dir.path(), false).unwrap();
        assert!(p.exists());
        assert!(service::is_installed(dir.path(), &spec.label));
        service::uninstall_from(dir.path(), &spec.label, false).unwrap();
        assert!(!service::is_installed(dir.path(), &spec.label));
    }
}
