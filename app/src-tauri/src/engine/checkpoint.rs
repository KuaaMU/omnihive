//! Checkpoint/Resume: persist task progress for crash recovery.
//!
//! On each step completion, a checkpoint is saved containing the task state,
//! completed step IDs, and a consensus snapshot. On resume, completed steps
//! are skipped via idempotency keys.

use crate::engine::task_model::Task;
use serde::{Deserialize, Serialize};
use std::path::Path;

const CHECKPOINT_FILE: &str = ".task.checkpoint.json";

// ===== Checkpoint =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub task_id: String,
    pub trace_id: String,
    pub completed_step_ids: Vec<String>,
    pub current_step_index: u32,
    pub consecutive_errors: u32,
    pub consensus_snapshot: String,
    pub saved_at: String,
}

impl Checkpoint {
    /// Create a checkpoint from a task and current consensus content.
    pub fn from_task(task: &Task, consensus: &str) -> Self {
        Self {
            task_id: task.task_id.clone(),
            trace_id: task.trace_id.clone(),
            completed_step_ids: task.completed_step_ids.clone(),
            current_step_index: task.current_step_index,
            consecutive_errors: task.consecutive_errors,
            consensus_snapshot: consensus.to_string(),
            saved_at: chrono::Local::now().format("%+").to_string(),
        }
    }

    /// Check whether a step has already been completed (for skip-on-resume).
    pub fn is_step_completed(&self, step_id: &str) -> bool {
        self.completed_step_ids.iter().any(|id| id == step_id)
    }

    /// Check whether a given step index has been completed.
    pub fn should_skip_step(&self, step_index: u32) -> bool {
        step_index < self.current_step_index
    }
}

// ===== Persistence =====

/// Save a checkpoint to .task.checkpoint.json
pub fn save_checkpoint(dir: &Path, checkpoint: &Checkpoint) -> Result<(), String> {
    let json = serde_json::to_string_pretty(checkpoint)
        .map_err(|e| format!("Failed to serialize checkpoint: {}", e))?;
    std::fs::write(dir.join(CHECKPOINT_FILE), json)
        .map_err(|e| format!("Failed to write checkpoint: {}", e))?;
    tracing::debug!(
        task_id = %checkpoint.task_id,
        step_index = checkpoint.current_step_index,
        "Checkpoint saved"
    );
    Ok(())
}

/// Load a checkpoint from .task.checkpoint.json
pub fn load_checkpoint(dir: &Path) -> Option<Checkpoint> {
    let path = dir.join(CHECKPOINT_FILE);
    if !path.exists() {
        return None;
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
}

/// Remove the checkpoint file (after successful task completion).
pub fn clear_checkpoint(dir: &Path) {
    let path = dir.join(CHECKPOINT_FILE);
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::task_model::Task;

    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("omnihive_test_checkpoint_{}", name));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn cleanup(dir: &std::path::PathBuf) {
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_checkpoint_from_task() {
        let task = Task::new("/tmp", "test", vec!["ceo".to_string()])
            .with_step_completed("step-1")
            .with_step_completed("step-2");
        let cp = Checkpoint::from_task(&task, "consensus content");

        assert_eq!(cp.task_id, task.task_id);
        assert_eq!(cp.completed_step_ids.len(), 2);
        assert_eq!(cp.current_step_index, 2);
        assert_eq!(cp.consensus_snapshot, "consensus content");
    }

    #[test]
    fn test_is_step_completed() {
        let task = Task::new("/tmp", "test", vec![]).with_step_completed("step-a");
        let cp = Checkpoint::from_task(&task, "");

        assert!(cp.is_step_completed("step-a"));
        assert!(!cp.is_step_completed("step-b"));
    }

    #[test]
    fn test_should_skip_step() {
        let task = Task::new("/tmp", "test", vec![])
            .with_step_completed("s1")
            .with_step_completed("s2")
            .with_step_completed("s3");
        let cp = Checkpoint::from_task(&task, "");

        assert!(cp.should_skip_step(0));
        assert!(cp.should_skip_step(1));
        assert!(cp.should_skip_step(2));
        assert!(!cp.should_skip_step(3));
    }

    #[test]
    fn test_save_and_load_checkpoint() {
        let dir = test_dir("save_load");
        let task = Task::new(dir.to_str().unwrap(), "test", vec!["ceo".to_string()])
            .with_step_completed("step-1");
        let cp = Checkpoint::from_task(&task, "## Company State\nAll good");

        save_checkpoint(&dir, &cp).unwrap();

        let loaded = load_checkpoint(&dir);
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.task_id, cp.task_id);
        assert_eq!(loaded.completed_step_ids, vec!["step-1"]);
        assert_eq!(loaded.consensus_snapshot, "## Company State\nAll good");

        cleanup(&dir);
    }

    #[test]
    fn test_load_checkpoint_nonexistent() {
        let dir = test_dir("nonexistent");
        let _ = std::fs::remove_file(dir.join(CHECKPOINT_FILE));
        assert!(load_checkpoint(&dir).is_none());
        cleanup(&dir);
    }

    #[test]
    fn test_clear_checkpoint() {
        let dir = test_dir("clear");
        let task = Task::new("/tmp", "test", vec![]);
        let cp = Checkpoint::from_task(&task, "");
        save_checkpoint(&dir, &cp).unwrap();
        assert!(dir.join(CHECKPOINT_FILE).exists());

        clear_checkpoint(&dir);
        assert!(!dir.join(CHECKPOINT_FILE).exists());

        cleanup(&dir);
    }

    #[test]
    fn test_checkpoint_serde_roundtrip() {
        let cp = Checkpoint {
            task_id: "t1".to_string(),
            trace_id: "tr1".to_string(),
            completed_step_ids: vec!["s1".to_string(), "s2".to_string()],
            current_step_index: 2,
            consecutive_errors: 0,
            consensus_snapshot: "snapshot".to_string(),
            saved_at: "2025-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&cp).unwrap();
        let parsed: Checkpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.task_id, "t1");
        assert_eq!(parsed.completed_step_ids.len(), 2);
    }

    #[test]
    fn test_resume_skips_completed_steps() {
        let dir = test_dir("resume");

        // Simulate: 3 steps completed, then crash
        let task = Task::new(
            dir.to_str().unwrap(),
            "test",
            vec!["a".into(), "b".into(), "c".into()],
        )
        .with_step_completed("s1")
        .with_step_completed("s2")
        .with_step_completed("s3");
        let cp = Checkpoint::from_task(&task, "consensus after step 3");
        save_checkpoint(&dir, &cp).unwrap();

        // Resume: load checkpoint
        let restored = load_checkpoint(&dir).unwrap();
        assert_eq!(restored.current_step_index, 3);

        // Steps 0, 1, 2 should be skipped; step 3 should not
        assert!(restored.should_skip_step(0));
        assert!(restored.should_skip_step(1));
        assert!(restored.should_skip_step(2));
        assert!(!restored.should_skip_step(3));

        cleanup(&dir);
    }
}
