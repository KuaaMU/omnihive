//! Task State Machine: immutable state transitions for the execution control plane.
//!
//! All transitions are pure functions that return a new state or an error.
//! Every valid transition can be traced.

use serde::{Deserialize, Serialize};
use std::fmt;

// ===== Task Status =====

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskStatus {
    Created,
    Planning,
    Running,
    Success,
    Failed,
    Cancelled,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Created => write!(f, "CREATED"),
            TaskStatus::Planning => write!(f, "PLANNING"),
            TaskStatus::Running => write!(f, "RUNNING"),
            TaskStatus::Success => write!(f, "SUCCESS"),
            TaskStatus::Failed => write!(f, "FAILED"),
            TaskStatus::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

// ===== Task Events =====

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskEvent {
    PlanStart,
    PlanComplete,
    AllStepsComplete,
    MaxRetriesExceeded,
    FatalError,
    UserCancel,
    Retry,
    Resume,
}

impl fmt::Display for TaskEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskEvent::PlanStart => write!(f, "plan_start"),
            TaskEvent::PlanComplete => write!(f, "plan_complete"),
            TaskEvent::AllStepsComplete => write!(f, "all_steps_complete"),
            TaskEvent::MaxRetriesExceeded => write!(f, "max_retries_exceeded"),
            TaskEvent::FatalError => write!(f, "fatal_error"),
            TaskEvent::UserCancel => write!(f, "user_cancel"),
            TaskEvent::Retry => write!(f, "retry"),
            TaskEvent::Resume => write!(f, "resume"),
        }
    }
}

// ===== Transition Error =====

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionError {
    pub from: TaskStatus,
    pub event: TaskEvent,
    pub message: String,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Invalid transition: {} + {} -> {}",
            self.from, self.event, self.message
        )
    }
}

impl std::error::Error for TransitionError {}

// ===== Transition Record =====

/// Immutable record of a state transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub from: TaskStatus,
    pub to: TaskStatus,
    pub event: TaskEvent,
    pub timestamp: String,
}

// ===== State Machine =====

/// Pure function: compute the next state given current state and event.
/// Returns the new status or a TransitionError if the transition is invalid.
pub fn transition(current: TaskStatus, event: TaskEvent) -> Result<TaskStatus, TransitionError> {
    let next = match (&current, &event) {
        // CREATED can start planning
        (TaskStatus::Created, TaskEvent::PlanStart) => TaskStatus::Planning,
        // CREATED can go directly to running (skip planning)
        (TaskStatus::Created, TaskEvent::PlanComplete) => TaskStatus::Running,
        // CREATED can be cancelled
        (TaskStatus::Created, TaskEvent::UserCancel) => TaskStatus::Cancelled,

        // PLANNING -> RUNNING when plan is complete
        (TaskStatus::Planning, TaskEvent::PlanComplete) => TaskStatus::Running,
        // PLANNING can be cancelled
        (TaskStatus::Planning, TaskEvent::UserCancel) => TaskStatus::Cancelled,

        // RUNNING -> terminal states
        (TaskStatus::Running, TaskEvent::AllStepsComplete) => TaskStatus::Success,
        (TaskStatus::Running, TaskEvent::MaxRetriesExceeded) => TaskStatus::Failed,
        (TaskStatus::Running, TaskEvent::FatalError) => TaskStatus::Failed,
        (TaskStatus::Running, TaskEvent::UserCancel) => TaskStatus::Cancelled,

        // Recovery: FAILED/CANCELLED -> RUNNING via retry/resume
        (TaskStatus::Failed, TaskEvent::Retry) => TaskStatus::Running,
        (TaskStatus::Failed, TaskEvent::Resume) => TaskStatus::Running,
        (TaskStatus::Cancelled, TaskEvent::Resume) => TaskStatus::Running,

        // Everything else is invalid
        _ => {
            return Err(TransitionError {
                from: current,
                event,
                message: "transition not allowed".to_string(),
            });
        }
    };

    Ok(next)
}

// ===== Step Status =====

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StepStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
    Blocked,
}

impl fmt::Display for StepStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StepStatus::Pending => write!(f, "PENDING"),
            StepStatus::Running => write!(f, "RUNNING"),
            StepStatus::Success => write!(f, "SUCCESS"),
            StepStatus::Failed => write!(f, "FAILED"),
            StepStatus::Skipped => write!(f, "SKIPPED"),
            StepStatus::Blocked => write!(f, "BLOCKED"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Valid transitions =====

    #[test]
    fn test_created_to_planning() {
        let result = transition(TaskStatus::Created, TaskEvent::PlanStart);
        assert_eq!(result, Ok(TaskStatus::Planning));
    }

    #[test]
    fn test_created_to_running_skip_planning() {
        let result = transition(TaskStatus::Created, TaskEvent::PlanComplete);
        assert_eq!(result, Ok(TaskStatus::Running));
    }

    #[test]
    fn test_created_to_cancelled() {
        let result = transition(TaskStatus::Created, TaskEvent::UserCancel);
        assert_eq!(result, Ok(TaskStatus::Cancelled));
    }

    #[test]
    fn test_planning_to_running() {
        let result = transition(TaskStatus::Planning, TaskEvent::PlanComplete);
        assert_eq!(result, Ok(TaskStatus::Running));
    }

    #[test]
    fn test_planning_to_cancelled() {
        let result = transition(TaskStatus::Planning, TaskEvent::UserCancel);
        assert_eq!(result, Ok(TaskStatus::Cancelled));
    }

    #[test]
    fn test_running_to_success() {
        let result = transition(TaskStatus::Running, TaskEvent::AllStepsComplete);
        assert_eq!(result, Ok(TaskStatus::Success));
    }

    #[test]
    fn test_running_to_failed_max_retries() {
        let result = transition(TaskStatus::Running, TaskEvent::MaxRetriesExceeded);
        assert_eq!(result, Ok(TaskStatus::Failed));
    }

    #[test]
    fn test_running_to_failed_fatal() {
        let result = transition(TaskStatus::Running, TaskEvent::FatalError);
        assert_eq!(result, Ok(TaskStatus::Failed));
    }

    #[test]
    fn test_running_to_cancelled() {
        let result = transition(TaskStatus::Running, TaskEvent::UserCancel);
        assert_eq!(result, Ok(TaskStatus::Cancelled));
    }

    #[test]
    fn test_failed_to_running_retry() {
        let result = transition(TaskStatus::Failed, TaskEvent::Retry);
        assert_eq!(result, Ok(TaskStatus::Running));
    }

    #[test]
    fn test_failed_to_running_resume() {
        let result = transition(TaskStatus::Failed, TaskEvent::Resume);
        assert_eq!(result, Ok(TaskStatus::Running));
    }

    #[test]
    fn test_cancelled_to_running_resume() {
        let result = transition(TaskStatus::Cancelled, TaskEvent::Resume);
        assert_eq!(result, Ok(TaskStatus::Running));
    }

    // ===== Invalid transitions =====

    #[test]
    fn test_success_cannot_transition() {
        for event in [
            TaskEvent::PlanStart,
            TaskEvent::PlanComplete,
            TaskEvent::AllStepsComplete,
            TaskEvent::MaxRetriesExceeded,
            TaskEvent::FatalError,
            TaskEvent::UserCancel,
            TaskEvent::Retry,
            TaskEvent::Resume,
        ] {
            let result = transition(TaskStatus::Success, event.clone());
            assert!(result.is_err(), "SUCCESS + {} should fail", event);
        }
    }

    #[test]
    fn test_created_cannot_retry() {
        let result = transition(TaskStatus::Created, TaskEvent::Retry);
        assert!(result.is_err());
    }

    #[test]
    fn test_created_cannot_complete_steps() {
        let result = transition(TaskStatus::Created, TaskEvent::AllStepsComplete);
        assert!(result.is_err());
    }

    #[test]
    fn test_planning_cannot_complete_steps() {
        let result = transition(TaskStatus::Planning, TaskEvent::AllStepsComplete);
        assert!(result.is_err());
    }

    #[test]
    fn test_running_cannot_plan_start() {
        let result = transition(TaskStatus::Running, TaskEvent::PlanStart);
        assert!(result.is_err());
    }

    #[test]
    fn test_failed_cannot_cancel() {
        let result = transition(TaskStatus::Failed, TaskEvent::UserCancel);
        assert!(result.is_err());
    }

    #[test]
    fn test_cancelled_cannot_retry() {
        let result = transition(TaskStatus::Cancelled, TaskEvent::Retry);
        assert!(result.is_err());
    }

    // ===== Error formatting =====

    #[test]
    fn test_transition_error_display() {
        let err = TransitionError {
            from: TaskStatus::Success,
            event: TaskEvent::Retry,
            message: "transition not allowed".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("SUCCESS"));
        assert!(msg.contains("retry"));
    }

    // ===== Serde roundtrip =====

    #[test]
    fn test_task_status_serde() {
        let status = TaskStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""RUNNING""#);
        let parsed: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, TaskStatus::Running);
    }

    #[test]
    fn test_task_event_serde() {
        let event = TaskEvent::AllStepsComplete;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#""all_steps_complete""#);
        let parsed: TaskEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, TaskEvent::AllStepsComplete);
    }

    #[test]
    fn test_step_status_serde() {
        let status = StepStatus::Blocked;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""BLOCKED""#);
        let parsed: StepStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, StepStatus::Blocked);
    }

    // ===== Full lifecycle test =====

    #[test]
    fn test_full_lifecycle_happy_path() {
        let mut status = TaskStatus::Created;
        status = transition(status, TaskEvent::PlanStart).unwrap();
        assert_eq!(status, TaskStatus::Planning);
        status = transition(status, TaskEvent::PlanComplete).unwrap();
        assert_eq!(status, TaskStatus::Running);
        status = transition(status, TaskEvent::AllStepsComplete).unwrap();
        assert_eq!(status, TaskStatus::Success);
    }

    #[test]
    fn test_full_lifecycle_fail_and_retry() {
        let mut status = TaskStatus::Created;
        status = transition(status, TaskEvent::PlanComplete).unwrap();
        assert_eq!(status, TaskStatus::Running);
        status = transition(status, TaskEvent::FatalError).unwrap();
        assert_eq!(status, TaskStatus::Failed);
        status = transition(status, TaskEvent::Retry).unwrap();
        assert_eq!(status, TaskStatus::Running);
        status = transition(status, TaskEvent::AllStepsComplete).unwrap();
        assert_eq!(status, TaskStatus::Success);
    }

    #[test]
    fn test_full_lifecycle_cancel_and_resume() {
        let mut status = TaskStatus::Created;
        status = transition(status, TaskEvent::PlanComplete).unwrap();
        status = transition(status, TaskEvent::UserCancel).unwrap();
        assert_eq!(status, TaskStatus::Cancelled);
        status = transition(status, TaskEvent::Resume).unwrap();
        assert_eq!(status, TaskStatus::Running);
        status = transition(status, TaskEvent::AllStepsComplete).unwrap();
        assert_eq!(status, TaskStatus::Success);
    }
}
