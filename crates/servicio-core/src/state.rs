use crate::error::CoreError;
use serde::{Deserialize, Serialize};

/// Lifecycle of a single running instance.
/// `Failed` = crash-loop tripped (gave up). `Stopped` = stopped by user/policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Crashed,
    Backoff,
    Idle,
    Completed,
    Failed,
}

impl InstanceState {
    pub fn is_terminal(self) -> bool {
        matches!(self, InstanceState::Stopped | InstanceState::Failed | InstanceState::Completed)
    }

    pub fn can_transition_to(self, to: InstanceState) -> bool {
        use InstanceState::*;
        matches!(
            (self, to),
            (Stopped, Starting)
                | (Starting, Running)
                | (Starting, Crashed)
                | (Running, Stopping)
                | (Running, Crashed)
                | (Stopping, Stopped)
                | (Crashed, Backoff)
                | (Crashed, Failed)
                | (Backoff, Starting)
                | (Backoff, Stopped)
                | (Running, Stopped)
                | (Stopped, Failed)
                | (Starting, Stopped)
                | (Running, Idle)
                | (Idle, Starting)
                | (Idle, Stopped)
                | (Running, Completed)
                | (Starting, Completed)
        )
    }

    pub fn transition_to(self, to: InstanceState) -> Result<InstanceState, CoreError> {
        if self.can_transition_to(to) {
            Ok(to)
        } else {
            Err(CoreError::BadTransition { from: format!("{self:?}"), to: format!("{to:?}") })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_path_running_to_crashed_to_backoff() {
        assert!(InstanceState::Starting.can_transition_to(InstanceState::Running));
        assert!(InstanceState::Running.can_transition_to(InstanceState::Crashed));
        assert!(InstanceState::Crashed.can_transition_to(InstanceState::Backoff));
        assert!(InstanceState::Backoff.can_transition_to(InstanceState::Starting));
    }

    #[test]
    fn clean_exit_and_log_failure_edges_are_legal() {
        assert!(InstanceState::Running.can_transition_to(InstanceState::Stopped));
        assert!(InstanceState::Stopped.can_transition_to(InstanceState::Failed));
    }

    #[test]
    fn illegal_transition_is_rejected() {
        assert!(!InstanceState::Stopped.can_transition_to(InstanceState::Running));
    }

    #[test]
    fn transition_returns_error_when_illegal() {
        let err = InstanceState::Stopped.transition_to(InstanceState::Running).unwrap_err();
        assert!(matches!(err, CoreError::BadTransition { .. }));
    }

    #[test]
    fn is_terminal_only_for_stopped_and_failed() {
        assert!(InstanceState::Stopped.is_terminal());
        assert!(InstanceState::Failed.is_terminal());
        assert!(!InstanceState::Running.is_terminal());
    }

    #[test]
    fn scheduled_idle_run_cycle_is_legal() {
        assert!(InstanceState::Running.can_transition_to(InstanceState::Idle));
        assert!(InstanceState::Idle.can_transition_to(InstanceState::Starting));
    }

    #[test]
    fn batch_completion_is_terminal() {
        assert!(InstanceState::Running.can_transition_to(InstanceState::Completed));
        assert!(InstanceState::Completed.is_terminal());
    }
}
