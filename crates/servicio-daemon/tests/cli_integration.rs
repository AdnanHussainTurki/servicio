// End-to-end: add a worker via the library API the CLI uses, confirm it persists,
// and confirm reconcile picks up exactly the autostart workers.
use servicio_daemon_lib::{add_worker, reconcile_specs};
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn add_then_reconcile_loads_autostart_worker() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("servicio.db");

    add_worker(
        &db_path,
        "queue",
        "sh",
        &["-c".into(), "sleep 1".into()],
        &PathBuf::from("/"),
        2,
        true,
    )
    .unwrap();

    let specs = reconcile_specs(&db_path).unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].name, "queue");
}
