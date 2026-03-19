//! Task Runner: executes a task through its lifecycle using tool adapters.
//!
//! The runner creates a Task, transitions through the state machine,
//! executes steps with tool adapters, writes trace events, and checkpoints progress.

use crate::checkpoint::{self, Checkpoint};
use crate::policy_engine::PolicyEngine;
use crate::state_machine::{self, TaskEvent, TaskStatus};
use crate::task_model::{self, Step, Task};
use crate::tool_protocol::{ExecutionContext, ToolInput, ToolRegistry};
use crate::trace_export::{self, TraceEvent};
use std::path::Path;
use std::sync::Arc;

/// Configuration for a task submission.
#[derive(Debug, Clone)]
pub struct SubmitConfig {
    pub goal: String,
    pub budget: Option<f64>,
    pub max_steps: u32,
    pub policy: PolicyMode,
    pub agents: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum PolicyMode {
    Permissive,
    Default,
}

impl Default for SubmitConfig {
    fn default() -> Self {
        Self {
            goal: String::new(),
            budget: None,
            max_steps: 20,
            policy: PolicyMode::Permissive,
            agents: vec!["default".to_string()],
        }
    }
}

/// Result of running a task.
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub trace_id: String,
    pub status: TaskStatus,
    pub steps_completed: u32,
    pub total_cost: f64,
    pub trace_file: String,
}

/// Run a task to completion in the given project directory.
///
/// This is the core execution loop that:
/// 1. Creates a Task
/// 2. Transitions CREATED -> RUNNING
/// 3. Executes steps using the tool registry
/// 4. Checkpoints after each step
/// 5. Writes trace events to JSONL
/// 6. Returns the final result
pub fn run_task(
    project_dir: &Path,
    config: &SubmitConfig,
    registry: &ToolRegistry,
) -> Result<TaskResult, String> {
    // Create task
    let mut task = Task::new(
        project_dir.to_str().unwrap_or("."),
        &config.goal,
        config.agents.clone(),
    );
    if let Some(budget) = config.budget {
        task.budget = Some(budget);
    }

    let trace_file = project_dir.join("logs").join("trace.jsonl");
    if let Some(parent) = trace_file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create logs directory: {}", e))?;
    }

    // Emit task_created trace
    emit_trace(&trace_file, &task, "task_created", None)?;

    // Transition to RUNNING
    let new_status = state_machine::transition(task.status, TaskEvent::PlanComplete)
        .map_err(|e| format!("State transition error: {}", e))?;
    task = task.with_status(new_status);
    task_model::write_task_state(project_dir, &task)?;

    emit_trace(&trace_file, &task, "task_running", None)?;

    // Build policy engine
    let policy = match config.policy {
        PolicyMode::Permissive => Arc::new(PolicyEngine::permissive()),
        PolicyMode::Default => Arc::new(PolicyEngine::from_guardrails(
            &[],
            project_dir.to_str().unwrap_or("."),
        )),
    };

    // Check for existing checkpoint to resume from
    let existing_checkpoint = checkpoint::load_checkpoint(project_dir);
    let start_step = existing_checkpoint
        .as_ref()
        .map(|cp| cp.completed_step_ids.len() as u32)
        .unwrap_or(0);

    let mut total_cost = 0.0f64;

    // Execute steps
    for step_index in start_step..config.max_steps {
        // Check budget
        if let Some(budget) = config.budget {
            if total_cost >= budget {
                emit_trace(&trace_file, &task, "budget_exceeded", None)?;
                task = task.with_status(TaskStatus::Failed);
                task = task.with_error(&format!(
                    "Budget exhausted: ${:.4} >= ${:.4}",
                    total_cost, budget
                ));
                task_model::write_task_state(project_dir, &task)?;
                break;
            }
        }

        // Pick agent for this step
        let agent = if config.agents.is_empty() {
            "default"
        } else {
            &config.agents[step_index as usize % config.agents.len()]
        };

        // Create step
        let step = Step::new(&task.task_id, agent);

        let ctx = ExecutionContext::new(
            &task.task_id,
            &step.step_id,
            &task.trace_id,
            agent,
            Arc::clone(&policy),
            project_dir.to_str().unwrap_or("."),
        );

        emit_trace_step(&trace_file, &task, &step, "step_started", agent, None, None)?;

        // Try to execute a tool call
        let tool_names = registry.list();
        if tool_names.is_empty() {
            // No tools registered - mark task complete
            emit_trace(&trace_file, &task, "no_tools_available", None)?;
            task = task.with_status(TaskStatus::Success);
            task_model::write_task_state(project_dir, &task)?;
            break;
        }

        // Execute with the first available tool as a demonstration
        // In a real system, the LLM agent decides which tool to call
        let input = ToolInput {
            tool_name: tool_names[0].to_string(),
            params: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "command".to_string(),
                    serde_json::json!(format!(
                        "echo 'Step {} for goal: {}'",
                        step_index, config.goal
                    )),
                );
                m
            },
        };

        match registry.execute(&input, &ctx) {
            Ok(output) => {
                let completed_step = step.completed(0, 0, &format!("{:?}", output.data));
                task = task.with_step_completed(&completed_step.step_id);

                if let Some(cost) = output.metadata.get("cost").and_then(|v| v.as_f64()) {
                    total_cost += cost;
                }

                emit_trace_step(
                    &trace_file,
                    &task,
                    &completed_step,
                    "step_completed",
                    agent,
                    None,
                    None,
                )?;
            }
            Err(tool_err) => {
                let failed_step = step.failed(&tool_err.message);
                task = task.with_error(&tool_err.message);

                emit_trace_step(
                    &trace_file,
                    &task,
                    &failed_step,
                    "step_failed",
                    agent,
                    None,
                    None,
                )?;

                if task.consecutive_errors >= 3 {
                    let new_status =
                        state_machine::transition(task.status, TaskEvent::MaxRetriesExceeded)
                            .map_err(|e| format!("State transition error: {}", e))?;
                    task = task.with_status(new_status);
                    task_model::write_task_state(project_dir, &task)?;
                    emit_trace(&trace_file, &task, "task_failed", None)?;
                    break;
                }
            }
        }

        // Checkpoint after each step
        let cp = Checkpoint::from_task(&task, "");
        checkpoint::save_checkpoint(project_dir, &cp)?;
        task_model::write_task_state(project_dir, &task)?;
    }

    // If we exhausted max_steps without failure, mark success
    if task.status == TaskStatus::Running {
        let new_status = state_machine::transition(task.status, TaskEvent::AllStepsComplete)
            .map_err(|e| format!("State transition error: {}", e))?;
        task = task.with_status(new_status);
        task_model::write_task_state(project_dir, &task)?;
        emit_trace(&trace_file, &task, "task_completed", None)?;
    }

    // Cleanup checkpoint on completion
    if matches!(task.status, TaskStatus::Success | TaskStatus::Failed) {
        checkpoint::clear_checkpoint(project_dir);
    }

    Ok(TaskResult {
        task_id: task.task_id.clone(),
        trace_id: task.trace_id.clone(),
        status: task.status,
        steps_completed: task.current_step_index,
        total_cost,
        trace_file: trace_file.display().to_string(),
    })
}

fn emit_trace(
    trace_file: &Path,
    task: &Task,
    event_type: &str,
    cost: Option<f64>,
) -> Result<(), String> {
    let mut event = TraceEvent::new(&task.trace_id, event_type).with_task(&task.task_id);
    if let Some(c) = cost {
        event = event.with_cost(c);
    }
    trace_export::append_trace_event(trace_file, &event)
}

fn emit_trace_step(
    trace_file: &Path,
    task: &Task,
    step: &Step,
    event_type: &str,
    agent: &str,
    latency_ms: Option<u64>,
    cost: Option<f64>,
) -> Result<(), String> {
    let mut event = TraceEvent::new(&task.trace_id, event_type)
        .with_task(&task.task_id)
        .with_step(&step.step_id)
        .with_payload(serde_json::json!({"agent": agent}));
    if let Some(l) = latency_ms {
        event = event.with_latency(l);
    }
    if let Some(c) = cost {
        event = event.with_cost(c);
    }
    trace_export::append_trace_event(trace_file, &event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_protocol::{Tool, ToolError, ToolOutput, ToolSchema};

    struct EchoTool;

    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        fn schema(&self) -> ToolSchema {
            ToolSchema {
                tool_id: "echo-v1".to_string(),
                name: "echo".to_string(),
                description: "Echo".to_string(),
                input_schema: serde_json::json!({}),
                output_schema: serde_json::json!({}),
                permissions: vec![],
                timeout_ms: 5000,
                idempotent: true,
            }
        }
        fn execute(
            &self,
            input: &ToolInput,
            _ctx: &ExecutionContext,
        ) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::ok(serde_json::json!({"echoed": input.params})))
        }
    }

    struct FailTool;

    impl Tool for FailTool {
        fn name(&self) -> &str {
            "fail"
        }
        fn schema(&self) -> ToolSchema {
            ToolSchema {
                tool_id: "fail-v1".to_string(),
                name: "fail".to_string(),
                description: "Always fails".to_string(),
                input_schema: serde_json::json!({}),
                output_schema: serde_json::json!({}),
                permissions: vec![],
                timeout_ms: 5000,
                idempotent: false,
            }
        }
        fn execute(
            &self,
            _input: &ToolInput,
            _ctx: &ExecutionContext,
        ) -> Result<ToolOutput, ToolError> {
            Err(ToolError::execution_failed("intentional failure"))
        }
    }

    #[test]
    fn test_run_task_success() {
        let dir = std::env::temp_dir().join("omnihive_test_runner_success");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));

        let config = SubmitConfig {
            goal: "test goal".to_string(),
            max_steps: 3,
            ..Default::default()
        };

        let result = run_task(&dir, &config, &registry).unwrap();
        assert_eq!(result.status, TaskStatus::Success);
        assert_eq!(result.steps_completed, 3);

        // Verify trace file exists
        assert!(dir.join("logs").join("trace.jsonl").exists());

        // Verify task state
        let task = task_model::read_task_state(&dir).unwrap();
        assert_eq!(task.status, TaskStatus::Success);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_run_task_failure_max_retries() {
        let dir = std::env::temp_dir().join("omnihive_test_runner_fail");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(FailTool));

        let config = SubmitConfig {
            goal: "doomed goal".to_string(),
            max_steps: 10,
            ..Default::default()
        };

        let result = run_task(&dir, &config, &registry).unwrap();
        assert_eq!(result.status, TaskStatus::Failed);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_run_task_no_tools() {
        let dir = std::env::temp_dir().join("omnihive_test_runner_no_tools");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);

        let registry = ToolRegistry::new();
        let config = SubmitConfig {
            goal: "test".to_string(),
            max_steps: 5,
            ..Default::default()
        };

        let result = run_task(&dir, &config, &registry).unwrap();
        assert_eq!(result.status, TaskStatus::Success);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_run_task_budget_exceeded() {
        let dir = std::env::temp_dir().join("omnihive_test_runner_budget");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));

        let config = SubmitConfig {
            goal: "budget test".to_string(),
            budget: Some(0.0), // zero budget = immediate exhaustion
            max_steps: 5,
            ..Default::default()
        };

        let result = run_task(&dir, &config, &registry).unwrap();
        // Budget check triggers before first step executes
        assert_eq!(result.status, TaskStatus::Failed);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_run_task_writes_checkpoint() {
        let dir = std::env::temp_dir().join("omnihive_test_runner_checkpoint");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));

        let config = SubmitConfig {
            goal: "checkpoint test".to_string(),
            max_steps: 2,
            ..Default::default()
        };

        let result = run_task(&dir, &config, &registry).unwrap();
        assert_eq!(result.status, TaskStatus::Success);

        // Checkpoint should be cleared on success
        assert!(checkpoint::load_checkpoint(&dir).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_run_task_trace_events() {
        let dir = std::env::temp_dir().join("omnihive_test_runner_trace");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));

        let config = SubmitConfig {
            goal: "trace test".to_string(),
            max_steps: 1,
            ..Default::default()
        };

        let result = run_task(&dir, &config, &registry).unwrap();
        let trace_path = dir.join("logs").join("trace.jsonl");
        let events = trace_export::read_trace_events(&trace_path).unwrap();

        // Should have: task_created, task_running, step_started, step_completed, task_completed
        assert!(events.len() >= 4);
        assert_eq!(events[0].event_type, "task_created");
        assert_eq!(events[0].task_id, Some(result.task_id));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_submit_config_default() {
        let config = SubmitConfig::default();
        assert_eq!(config.max_steps, 20);
        assert!(config.budget.is_none());
        assert!(config.goal.is_empty());
    }
}
