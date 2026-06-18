use rusqlite::Connection;
use servicio_core::worker::WorkerSpec;
use std::path::Path;

/// A single metric sample: (timestamp, cpu, mem).
pub type MetricRow = (u64, f32, u64);
/// Metric samples for one instance: (instance, points).
pub type InstanceSeries = (u32, Vec<MetricRow>);

/// SQLite persistence for worker definitions. Source of truth for the daemon.
pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        // rusqlite/SQLite won't create missing parent dirs; do it so a fresh
        // base dir (e.g. `add --db /tmp/servicio/servicio.db`) works first time.
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
                        Some(format!("create db dir {}: {e}", parent.display())),
                    )
                })?;
            }
        }
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
            );
            CREATE TABLE IF NOT EXISTS metrics (
                worker   TEXT NOT NULL,
                instance INTEGER NOT NULL,
                ts       INTEGER NOT NULL,
                cpu      REAL NOT NULL,
                mem      INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_metrics_worker_ts ON metrics(worker, ts);",
        )
    }

    pub fn insert_metric(
        &self,
        worker: &str,
        instance: u32,
        ts: u64,
        cpu: f32,
        mem: u64,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO metrics (worker, instance, ts, cpu, mem) VALUES (?1,?2,?3,?4,?5)",
            rusqlite::params![worker, instance, ts as i64, cpu as f64, mem as i64],
        )?;
        Ok(())
    }

    /// Returns (instance, points) grouped, points = (ts,cpu,mem), for ts >= since.
    pub fn query_metrics(&self, worker: &str, since: u64) -> rusqlite::Result<Vec<InstanceSeries>> {
        let mut stmt = self.conn.prepare(
            "SELECT instance, ts, cpu, mem FROM metrics WHERE worker=?1 AND ts>=?2 ORDER BY instance, ts")?;
        let rows = stmt.query_map(rusqlite::params![worker, since as i64], |r| {
            Ok((
                r.get::<_, i64>(0)? as u32,
                r.get::<_, i64>(1)? as u64,
                r.get::<_, f64>(2)? as f32,
                r.get::<_, i64>(3)? as u64,
            ))
        })?;
        let mut out: Vec<InstanceSeries> = Vec::new();
        for row in rows {
            let (inst, ts, cpu, mem) = row?;
            match out.last_mut() {
                Some((i, pts)) if *i == inst => pts.push((ts, cpu, mem)),
                _ => out.push((inst, vec![(ts, cpu, mem)])),
            }
        }
        Ok(out)
    }

    pub fn prune_metrics(&self, older_than_ts: u64) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM metrics WHERE ts < ?1", [older_than_ts as i64])?;
        Ok(())
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
        self.query(
            "SELECT spec_json FROM workers WHERE autostart = 1 AND enabled = 1 ORDER BY name",
        )
    }

    /// Fetch one worker by name.
    pub fn get_worker(&self, name: &str) -> rusqlite::Result<Option<WorkerSpec>> {
        let mut stmt = self
            .conn
            .prepare("SELECT spec_json FROM workers WHERE name = ?1")?;
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
        let n = self
            .conn
            .execute("DELETE FROM workers WHERE name = ?1", [name])?;
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
            group: None,
            tags: Vec::new(),
            display_name: None,
        }
    }

    #[test]
    fn open_creates_missing_parent_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/sub/servicio.db");
        let db = Db::open(&path).unwrap();
        db.upsert_worker(&spec("q")).unwrap();
        assert!(path.exists());
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
        let names: Vec<_> = db
            .autostart_workers()
            .unwrap()
            .into_iter()
            .map(|w| w.name)
            .collect();
        assert_eq!(names, vec!["yes".to_string()]);
    }

    #[test]
    fn metrics_insert_query_and_prune() {
        let db = Db::open_in_memory().unwrap();
        db.insert_metric("q", 0, 100, 1.5, 1000).unwrap();
        db.insert_metric("q", 0, 200, 2.5, 2000).unwrap();
        db.insert_metric("q", 1, 200, 0.5, 500).unwrap();
        let series = db.query_metrics("q", 0).unwrap();
        let s0 = series.iter().find(|s| s.0 == 0).unwrap();
        assert_eq!(s0.1.len(), 2);
        db.prune_metrics(150).unwrap();
        let after = db.query_metrics("q", 0).unwrap();
        let s0 = after.iter().find(|s| s.0 == 0).unwrap();
        assert_eq!(s0.1.len(), 1);
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
