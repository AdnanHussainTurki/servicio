use crate::state::InstanceState;
use crate::worker::RunMode;
use serde::{Deserialize, Serialize};

/// An event emitted by a supervisor onto the manager's broadcast channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SupervisorEvent {
    State { worker: String, instance: u32, from: InstanceState, to: InstanceState },
    Log { worker: String, instance: u32, stream: String, line: String },
}

/// Snapshot of one instance for status reporting.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceStatus {
    pub index: u32,
    pub state: InstanceState,
    pub restart_count: u32,
    pub pid: Option<u32>,
}

/// Snapshot of one worker (all its instances).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkerStatusCore {
    pub name: String,
    pub run_mode: RunMode,
    pub instances: Vec<InstanceStatus>,
}
