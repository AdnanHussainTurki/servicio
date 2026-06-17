use serde::{Deserialize, Serialize};
use servicio_core::state::InstanceState;
use servicio_core::worker::{RunMode, WorkerSpec};

/// Status of one worker as reported by `list_workers`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerStatus {
    pub name: String,
    pub run_mode: RunMode,
    pub instances: Vec<InstanceStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceStatus {
    pub index: u32,
    pub state: InstanceState,
    pub restart_count: u32,
    pub pid: Option<u32>,
}

/// Params for `add_worker` — a full worker definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddWorkerParams {
    pub spec: WorkerSpec,
}

/// Params for the single-name methods (start/stop/remove).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NameParams {
    pub name: String,
}

/// Params for `hello`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HelloParams {
    pub token: String,
}

/// Params for `subscribe`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubscribeParams {
    pub topics: Vec<String>,
    #[serde(default)]
    pub worker: Option<String>,
}

/// Event payloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateEvent {
    pub worker: String,
    pub instance: u32,
    pub from: InstanceState,
    pub to: InstanceState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEvent {
    pub worker: String,
    pub instance: u32,
    pub stream: String,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonInfo {
    pub version: String,
    pub uptime_secs: u64,
    pub worker_count: u32,
    pub running_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn worker_status_roundtrips() {
        let s = WorkerStatus {
            name: "q".into(),
            run_mode: RunMode::Daemon { concurrency: 2 },
            instances: vec![InstanceStatus {
                index: 0,
                state: InstanceState::Running,
                restart_count: 1,
                pid: Some(4321),
            }],
        };
        let v = serde_json::to_value(&s).unwrap();
        let back: WorkerStatus = serde_json::from_value(v).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn subscribe_params_default_worker_is_none() {
        let p: SubscribeParams = serde_json::from_value(json!({"topics": ["state"]})).unwrap();
        assert_eq!(p.worker, None);
    }

    #[test]
    fn state_event_roundtrips() {
        let e = StateEvent {
            worker: "q".into(),
            instance: 0,
            from: InstanceState::Starting,
            to: InstanceState::Running,
        };
        let back: StateEvent = serde_json::from_value(serde_json::to_value(&e).unwrap()).unwrap();
        assert_eq!(e, back);
    }
}
