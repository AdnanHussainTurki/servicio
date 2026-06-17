use rusqlite::Connection;
use servicio_core::worker::WorkerSpec;
use std::path::Path;

/// SQLite persistence for worker definitions. Source of truth for the daemon.
pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS workers (
                name      TEXT PRIMARY KEY,
                spec_json TEXT NOT NULL,
                autostart INTEGER NOT NULL,
                enabled   INTEGER NOT NULL
            );",
        )
    }

    /// Insert or replace a worker by name. Full spec stored as JSON; a couple of
    /// columns are duplicated for cheap filtering.
    pub fn upsert_worker(&self, spec: &WorkerSpec) -> rusqlite::Result<()> {
        let json = serde_json::to_string(spec).expect("spec serializes");
        self.conn.execute(
            "INSERT INTO workers (name, spec_json, autostart, enabled)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(name) DO UPDATE SET
                spec_json = excluded.spec_json,
                autostart = excluded.autostart,
                enabled   = excluded.enabled",
            rusqlite::params![spec.name, json, spec.autostart as i64, spec.enabled as i64],
        )?;
        Ok(())
    }

    pub fn list_workers(&self) -> rusqlite::Result<Vec<WorkerSpec>> {
        self.query("SELECT spec_json FROM workers ORDER BY name")
    }

    /// Workers that should be (re)started automatically by the daemon.
    pub fn autostart_workers(&self) -> rusqlite::Result<Vec<WorkerSpec>> {
        self.query("SELECT spec_json FROM workers WHERE autostart = 1 AND enabled = 1 ORDER BY name")
    }

    /// Fetch one worker by name.
    pub fn get_worker(&self, name: &str) -> rusqlite::Result<Option<WorkerSpec>> {
        let mut stmt = self.conn.prepare("SELECT spec_json FROM workers WHERE name = ?1")?;
        let mut rows = stmt.query_map([name], |row| {
            let json: String = row.get(0)?;
            Ok(serde_json::from_str::<WorkerSpec>(&json).expect("stored spec parses"))
        })?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// Delete a worker by name; returns true if a row was removed.
    pub fn remove_worker(&self, name: &str) -> rusqlite::Result<bool> {
        let n = self.conn.execute("DELETE FROM workers WHERE name = ?1", [name])?;
        Ok(n > 0)
    }

    fn query(&self, sql: &str) -> rusqlite::Result<Vec<WorkerSpec>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            let json: String = row.get(0)?;
            Ok(serde_json::from_str::<WorkerSpec>(&json).expect("stored spec parses"))
        })?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use servicio_core::worker::{RestartPolicy, RunMode};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn spec(name: &str) -> WorkerSpec {
        WorkerSpec {
            name: name.into(),
            command: "sh".into(),
            args: vec!["-c".into(), "sleep 1".into()],
            working_dir: PathBuf::from("/"),
            env: BTreeMap::new(),
            run_mode: RunMode::Daemon { concurrency: 2 },
            restart: RestartPolicy::default(),
            autostart: true,
            enabled: true,
        }
    }

    #[test]
    fn migrate_then_upsert_and_list_roundtrips() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_worker(&spec("queue")).unwrap();
        let all = db.list_workers().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "queue");
        assert_eq!(all[0].run_mode, RunMode::Daemon { concurrency: 2 });
    }

    #[test]
    fn upsert_same_name_replaces_not_duplicates() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_worker(&spec("queue")).unwrap();
        let mut changed = spec("queue");
        changed.autostart = false;
        db.upsert_worker(&changed).unwrap();
        let all = db.list_workers().unwrap();
        assert_eq!(all.len(), 1);
        assert!(!all[0].autostart);
    }

    #[test]
    fn autostart_filter_returns_only_autostart_enabled() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_worker(&spec("yes")).unwrap();
        let mut no = spec("no");
        no.autostart = false;
        db.upsert_worker(&no).unwrap();
        let names: Vec<_> = db.autostart_workers().unwrap().into_iter().map(|w| w.name).collect();
        assert_eq!(names, vec!["yes".to_string()]);
    }

    #[test]
    fn get_and_remove_worker() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_worker(&spec("q")).unwrap();
        assert!(db.get_worker("q").unwrap().is_some());
        assert!(db.remove_worker("q").unwrap());
        assert!(db.get_worker("q").unwrap().is_none());
        assert!(!db.remove_worker("q").unwrap());
    }
}
