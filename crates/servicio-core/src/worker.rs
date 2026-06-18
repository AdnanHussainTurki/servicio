use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn default_concurrency() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Schedule {
    Cron(String),
    IntervalSecs(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlapPolicy {
    Skip,
    Queue,
    KillPrevious,
}

/// How a worker is run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunMode {
    Daemon {
        #[serde(default = "default_concurrency")]
        concurrency: u32,
    },
    Scheduled {
        schedule: Schedule,
        #[serde(default = "default_overlap")]
        overlap: OverlapPolicy,
    },
    Batch {
        run_count: u32,
        #[serde(default)]
        delay_secs: u64,
    },
}

fn default_overlap() -> OverlapPolicy {
    OverlapPolicy::Skip
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestartKind {
    Always,
    OnFailure,
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestartPolicy {
    pub kind: RestartKind,
    pub max_retries: u32,
    /// base/max/reset are seconds; mapped to Backoff by the supervisor.
    pub base_secs: u64,
    pub max_secs: u64,
    pub reset_window_secs: u64,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            kind: RestartKind::OnFailure,
            max_retries: 5,
            base_secs: 1,
            max_secs: 60,
            reset_window_secs: 30,
        }
    }
}

/// A worker definition. The unit a user creates; the manager spawns instances from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerSpec {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub run_mode: RunMode,
    #[serde(default)]
    pub restart: RestartPolicy,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub display_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_spec_roundtrips_through_json() {
        let spec = WorkerSpec {
            name: "laravel-queue".into(),
            command: "php".into(),
            args: vec!["artisan".into(), "queue:work".into()],
            working_dir: PathBuf::from("/tmp"),
            env: BTreeMap::new(),
            run_mode: RunMode::Daemon { concurrency: 4 },
            restart: RestartPolicy::default(),
            autostart: true,
            enabled: true,
            group: None,
            tags: Vec::new(),
            display_name: None,
        };
        let json = serde_json::to_string(&spec).unwrap();
        let back: WorkerSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, back);
    }

    #[test]
    fn worker_spec_group_tags_roundtrip() {
        let mut s = WorkerSpec {
            name: "q".into(),
            command: "sh".into(),
            args: vec![],
            working_dir: PathBuf::from("/"),
            env: BTreeMap::new(),
            run_mode: RunMode::Daemon { concurrency: 1 },
            restart: RestartPolicy::default(),
            autostart: false,
            enabled: true,
            group: Some("app".into()),
            tags: vec!["redis".into(), "critical".into()],
            display_name: None,
        };
        let back: WorkerSpec = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(s, back);
        s.group = None;
        s.tags = vec![];
        // back-compat: JSON without group/tags fields loads to defaults
        let old = r#"{"name":"q","command":"sh","args":[],"working_dir":"/","env":{},"run_mode":{"type":"daemon","concurrency":1},"restart":{"kind":"on_failure","max_retries":5,"base_secs":1,"max_secs":60,"reset_window_secs":30},"autostart":false,"enabled":true}"#;
        let loaded: WorkerSpec = serde_json::from_str(old).unwrap();
        assert_eq!(loaded.group, None);
        assert!(loaded.tags.is_empty());
    }

    #[test]
    fn worker_spec_display_name_roundtrip() {
        let s = WorkerSpec {
            name: "q".into(),
            command: "sh".into(),
            args: vec![],
            working_dir: PathBuf::from("/"),
            env: BTreeMap::new(),
            run_mode: RunMode::Daemon { concurrency: 1 },
            restart: RestartPolicy::default(),
            autostart: false,
            enabled: true,
            group: None,
            tags: vec![],
            display_name: Some("Queue Worker".into()),
        };
        let back: WorkerSpec = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(s, back);
        assert_eq!(back.display_name.as_deref(), Some("Queue Worker"));
        // back-compat: JSON without display_name field loads to None
        let old = r#"{"name":"q","command":"sh","args":[],"working_dir":"/","env":{},"run_mode":{"type":"daemon","concurrency":1},"restart":{"kind":"on_failure","max_retries":5,"base_secs":1,"max_secs":60,"reset_window_secs":30},"autostart":false,"enabled":true}"#;
        let loaded: WorkerSpec = serde_json::from_str(old).unwrap();
        assert_eq!(loaded.display_name, None);
    }

    #[test]
    fn default_restart_policy_is_on_failure() {
        let p = RestartPolicy::default();
        assert_eq!(p.kind, RestartKind::OnFailure);
        assert_eq!(p.max_retries, 5);
    }

    #[test]
    fn concurrency_defaults_to_one_when_unset() {
        let mode: RunMode = serde_json::from_str(r#"{"type":"daemon"}"#).unwrap();
        assert_eq!(mode, RunMode::Daemon { concurrency: 1 });
    }

    #[test]
    fn scheduled_mode_roundtrips() {
        let m = RunMode::Scheduled {
            schedule: Schedule::Cron("0 3 * * *".into()),
            overlap: OverlapPolicy::Skip,
        };
        let back: RunMode = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn batch_mode_roundtrips() {
        let m = RunMode::Batch {
            run_count: 5,
            delay_secs: 10,
        };
        let back: RunMode = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn interval_schedule_roundtrips() {
        let s = Schedule::IntervalSecs(30);
        let back: Schedule = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(s, back);
    }
}
