use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn default_concurrency() -> u32 { 1 }

/// How a worker is run. Phase 1 supports Daemon only; later phases add
/// Scheduled and Batch as new variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunMode {
    Daemon {
        #[serde(default = "default_concurrency")]
        concurrency: u32,
    },
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
        Self { kind: RestartKind::OnFailure, max_retries: 5, base_secs: 1, max_secs: 60, reset_window_secs: 30 }
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
        };
        let json = serde_json::to_string(&spec).unwrap();
        let back: WorkerSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, back);
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
}
