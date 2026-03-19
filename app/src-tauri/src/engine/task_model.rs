//! Task and Step data models for the execution control plane.
//!
//! These models map the autonomous loop to a structured Task/Step hierarchy,
//! enabling checkpoint/resume, trace correlation, and future CLI usage.

use serde::{Deserialize, Serialize};
use crate::engine::state_machine::{TaskStatus, StepStatus};

// ===== Task =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub task_id: String,
    pub goal: String,
    pub status: TaskStatus,
    pub trace_id: String,
    pub project_dir: String,
    pub agent_roles: Vec<String>,
    pub budget: Option<f64>,
    pub current_step_index: u32,
    pub total_steps: u32,
    pub consecutive_errors: u32,
    pub completed_step_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

impl Task {
    pub fn new(project_dir: &str, goal: &str, agent_roles: Vec<String>) -> Self {
        let now = chrono::Local::now().format("%+").to_string();
        Self {
            task_id: uuid::Uuid::new_v4().to_string(),
            goal: goal.to_string(),
            status: TaskStatus::Created,
            trace_id: uuid::Uuid::new_v4().to_string(),
            project_dir: project_dir.to_string(),
            agent_roles,
            budget: None,
            current_step_index: 0,
            total_steps: 0,
            consecutive_errors: 0,
            completed_step_ids: vec![],
            created_at: now.clone(),
            updated_at: now,
            completed_at: None,
            error: None,
        }
    }

    /// Create an updated copy with new status and timestamp.
    pub fn with_status(&self, status: TaskStatus) -> Self {
        let mut next = self.clone();
        next.status = status;
        next.updated_at = chrono::Local::now().format("%+").to_string();
        if matches!(status, TaskStatus::Success | TaskStatus::Failed | TaskStatus::Cancelled) {
            next.completed_at = Some(next.updated_at.clone());
        }
        next
    }

    /// Create an updated copy with incremented step index.
    pub fn with_step_completed(&self, step_id: &str) -> Self {
        let mut next = self.clone();
        next.current_step_index += 1;
        next.total_steps = next.current_step_index;
        next.consecutive_errors = 0;
        next.completed_step_ids.push(step_id.to_string());
        next.updated_at = chrono::Local::now().format("%+").to_string();
        next
    }

    /// Create an updated copy with incremented error count.
    pub fn with_error(&self, error: &str) -> Self {
        let mut next = self.clone();
        next.consecutive_errors += 1;
        next.error = Some(error.to_string());
        next.updated_at = chrono::Local::now().format("%+").to_string();
        next
    }
}

// ===== Step =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub step_id: String,
    pub task_id: String,
    pub agent_id: String,
    pub status: StepStatus,
    pub retry_count: u32,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
    #[serde(default)]
    pub response_preview: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    pub started_at: String,
    #[serde(default)]
    pub ended_at: Option<String>,
}

impl Step {
    pub fn new(task_id: &str, agent_id: &str) -> Self {
        Self {
            step_id: uuid::Uuid::new_v4().to_string(),
            task_id: task_id.to_string(),
            agent_id: agent_id.to_string(),
            status: StepStatus::Running,
            retry_count: 0,
            idempotency_key: None,
            input_tokens: 0,
            output_tokens: 0,
            response_preview: None,
            error: None,
            started_at: chrono::Local::now().format("%+").to_string(),
            ended_at: None,
        }
    }

    /// Create a completed copy with token counts and preview.
    pub fn completed(
        &self,
        input_tokens: u32,
        output_tokens: u32,
        preview: &str,
    ) -> Self {
        let mut next = self.clone();
        next.status = StepStatus::Success;
        next.input_tokens = input_tokens;
        next.output_tokens = output_tokens;
        next.response_preview = Some(preview.to_string());
        next.ended_at = Some(chrono::Local::now().format("%+").to_string());
        next
    }

    /// Create a failed copy with error message.
    pub fn failed(&self, error: &str) -> Self {
        let mut next = self.clone();
        next.status = StepStatus::Failed;
        next.error = Some(error.to_string());
        next.ended_at = Some(chrono::Local::now().format("%+").to_string());
        next
    }
}

// ===== Task State File I/O =====

use std::path::Path;

const TASK_STATE_FILE: &str = ".task.state.json";
const LEGACY_STATE_FILE: &str = ".loop.state";

/// Write task state as JSON to .task.state.json
pub fn write_task_state(dir: &Path, task: &Task) -> Result<(), String> {
    let json = serde_json::to_string_pretty(task)
        .map_err(|e| format!("Failed to serialize task state: {}", e))?;
    std::fs::write(dir.join(TASK_STATE_FILE), json)
        .map_err(|e| format!("Failed to write task state: {}", e))
}

/// Read task state from .task.state.json, or migrate from legacy .loop.state
pub fn read_task_state(dir: &Path) -> Option<Task> {
    let task_path = dir.join(TASK_STATE_FILE);
    if task_path.exists() {
        return std::fs::read_to_string(&task_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok());
    }

    // Backward compat: migrate from legacy .loop.state
    let legacy_path = dir.join(LEGACY_STATE_FILE);
    if legacy_path.exists() {
        return migrate_legacy_state(dir);
    }

    None
}

/// Migrate a legacy .loop.state file to .task.state.json
fn migrate_legacy_state(dir: &Path) -> Option<Task> {
    let legacy_path = dir.join(LEGACY_STATE_FILE);
    let content = std::fs::read_to_string(&legacy_path).ok()?;

    let mut current_cycle = 0u32;
    let mut total_cycles = 0u32;
    let mut consecutive_errors = 0u32;
    let mut status_str = "stopped";

    for line in content.lines() {
        if let Some(val) = line.strip_prefix("current_cycle=") {
            current_cycle = val.parse().unwrap_or(0);
        }
        if let Some(val) = line.strip_prefix("total_cycles=") {
            total_cycles = val.parse().unwrap_or(0);
        }
        if let Some(val) = line.strip_prefix("consecutive_errors=") {
            consecutive_errors = val.parse().unwrap_or(0);
        }
        if let Some(val) = line.strip_prefix("status=") {
            status_str = match val.trim() {
                "running" => "running",
                "error" => "error",
                _ => "stopped",
            };
        }
    }

    let task_status = match status_str {
        "running" => TaskStatus::Running,
        "error" => TaskStatus::Failed,
        _ => TaskStatus::Created,
    };

    let now = chrono::Local::now().format("%+").to_string();
    let task = Task {
        task_id: uuid::Uuid::new_v4().to_string(),
        goal: "(migrated from legacy state)".to_string(),
        status: task_status,
        trace_id: uuid::Uuid::new_v4().to_string(),
        project_dir: dir.display().to_string(),
        agent_roles: vec![],
        budget: None,
        current_step_index: current_cycle,
        total_steps: total_cycles,
        consecutive_errors,
        completed_step_ids: vec![],
        created_at: now.clone(),
        updated_at: now,
        completed_at: None,
        error: None,
    };

    // Write the migrated state
    if write_task_state(dir, &task).is_ok() {
        tracing::info!("Migrated legacy .loop.state to .task.state.json");
    }

    Some(task)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_new() {
        let task = Task::new("/tmp/test", "test goal", vec!["ceo".to_string()]);
        assert_eq!(task.status, TaskStatus::Created);
        assert_eq!(task.goal, "test goal");
        assert_eq!(task.agent_roles, vec!["ceo"]);
        assert_eq!(task.current_step_index, 0);
        assert_eq!(task.consecutive_errors, 0);
        assert!(!task.task_id.is_empty());
        assert!(!task.trace_id.is_empty());
    }

    #[test]
    fn test_task_with_status_immutable() {
        let original = Task::new("/tmp", "goal", vec![]);
        let updated = original.with_status(TaskStatus::Running);
        assert_eq!(original.status, TaskStatus::Created);
        assert_eq!(updated.status, TaskStatus::Running);
    }

    #[test]
    fn test_task_with_status_sets_completed_at() {
        let task = Task::new("/tmp", "goal", vec![]);
        let success = task.with_status(TaskStatus::Success);
        assert!(success.completed_at.is_some());

        let running = task.with_status(TaskStatus::Running);
        assert!(running.completed_at.is_none());
    }

    #[test]
    fn test_task_with_step_completed() {
        let task = Task::new("/tmp", "goal", vec![]);
        let updated = task.with_step_completed("step-1");
        assert_eq!(updated.current_step_index, 1);
        assert_eq!(updated.total_steps, 1);
        assert_eq!(updated.completed_step_ids, vec!["step-1"]);
        assert_eq!(updated.consecutive_errors, 0);
    }

    #[test]
    fn test_task_with_error() {
        let task = Task::new("/tmp", "goal", vec![]);
        let errored = task.with_error("something broke");
        assert_eq!(errored.consecutive_errors, 1);
        assert_eq!(errored.error, Some("something broke".to_string()));
    }

    #[test]
    fn test_step_new() {
        let step = Step::new("task-123", "ceo");
        assert_eq!(step.task_id, "task-123");
        assert_eq!(step.agent_id, "ceo");
        assert_eq!(step.status, StepStatus::Running);
        assert_eq!(step.retry_count, 0);
    }

    #[test]
    fn test_step_completed_immutable() {
        let step = Step::new("t", "a");
        let done = step.completed(100, 50, "preview text");
        assert_eq!(step.status, StepStatus::Running);
        assert_eq!(done.status, StepStatus::Success);
        assert_eq!(done.input_tokens, 100);
        assert_eq!(done.output_tokens, 50);
        assert!(done.ended_at.is_some());
    }

    #[test]
    fn test_step_failed_immutable() {
        let step = Step::new("t", "a");
        let fail = step.failed("timeout");
        assert_eq!(step.status, StepStatus::Running);
        assert_eq!(fail.status, StepStatus::Failed);
        assert_eq!(fail.error, Some("timeout".to_string()));
    }

    #[test]
    fn test_task_serde_roundtrip() {
        let task = Task::new("/tmp/test", "build app", vec!["ceo".to_string(), "devops".to_string()]);
        let json = serde_json::to_string(&task).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.task_id, task.task_id);
        assert_eq!(parsed.goal, "build app");
        assert_eq!(parsed.agent_roles.len(), 2);
    }

    #[test]
    fn test_step_serde_roundtrip() {
        let step = Step::new("task-1", "ceo").completed(500, 200, "decided to pivot");
        let json = serde_json::to_string(&step).unwrap();
        let parsed: Step = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.input_tokens, 500);
        assert_eq!(parsed.status, StepStatus::Success);
    }

    #[test]
    fn test_write_and_read_task_state() {
        let dir = std::env::temp_dir().join("omnihive_test_task_state");
        let _ = std::fs::create_dir_all(&dir);

        let task = Task::new(dir.to_str().unwrap(), "test", vec!["ceo".to_string()]);
        write_task_state(&dir, &task).unwrap();

        let loaded = read_task_state(&dir);
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.task_id, task.task_id);
        assert_eq!(loaded.goal, "test");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_migrate_legacy_state() {
        let dir = std::env::temp_dir().join("omnihive_test_migrate_legacy");
        let _ = std::fs::create_dir_all(&dir);

        // Write legacy .loop.state
        let legacy = "current_cycle=5\ntotal_cycles=10\nconsecutive_errors=1\nstatus=running\n";
        std::fs::write(dir.join(".loop.state"), legacy).unwrap();

        let task = read_task_state(&dir);
        assert!(task.is_some());
        let task = task.unwrap();
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.current_step_index, 5);
        assert_eq!(task.total_steps, 10);
        assert_eq!(task.consecutive_errors, 1);

        // Verify .task.state.json was created
        assert!(dir.join(".task.state.json").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_read_task_state_nonexistent() {
        let dir = std::env::temp_dir().join("omnihive_test_no_state");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::remove_file(dir.join(TASK_STATE_FILE));
        let _ = std::fs::remove_file(dir.join(LEGACY_STATE_FILE));

        assert!(read_task_state(&dir).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
