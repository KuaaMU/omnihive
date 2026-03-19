use omnihive_core::eval;
use omnihive_core::runner;
use omnihive_core::task_model;
use omnihive_core::trace_export;
use std::path::Path;

/// Show all active tasks in the current directory.
pub fn status() -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Failed to get cwd: {}", e))?;
    show_task_status(&cwd)
}

fn show_task_status(dir: &Path) -> Result<(), String> {
    // Check for .task.state.json
    if let Some(task) = task_model::read_task_state(dir) {
        println!("Task: {} ({})", &task.task_id[..8], task.status);
        println!("  Goal:   {}", task.goal);
        println!("  Steps:  {}/{}", task.current_step_index, task.total_steps);
        println!("  Errors: {}", task.consecutive_errors);
        println!("  Created: {}", task.created_at);
        println!("  Updated: {}", task.updated_at);
        if let Some(ref err) = task.error {
            println!("  Error:  {}", err);
        }
        return Ok(());
    }

    // Check for legacy .loop.state
    let legacy = dir.join(".loop.state");
    if legacy.exists() {
        let content =
            std::fs::read_to_string(&legacy).map_err(|e| format!("Failed to read state: {}", e))?;
        println!("Legacy loop state:");
        for line in content.lines() {
            println!("  {}", line);
        }
        return Ok(());
    }

    println!("No active tasks found in {}", dir.display());
    Ok(())
}

/// Replay trace events from a JSONL file.
pub fn replay(trace_file: &Path, task_id: Option<&str>) -> Result<(), String> {
    let events = trace_export::read_trace_events(trace_file)?;

    if events.is_empty() {
        println!("No trace events found in {}", trace_file.display());
        return Ok(());
    }

    if let Some(tid) = task_id {
        let summary = trace_export::build_replay_summary(&events, tid);
        print_replay_summary(&summary);
    } else {
        // List all unique task IDs
        let mut task_ids: Vec<String> = events.iter().filter_map(|e| e.task_id.clone()).collect();
        task_ids.sort();
        task_ids.dedup();

        if task_ids.is_empty() {
            println!("Total events: {} (no task IDs found)", events.len());
            for event in &events {
                println!(
                    "  [{}] {} {}",
                    event.timestamp,
                    event.event_type,
                    event.step_id.as_deref().unwrap_or("")
                );
            }
        } else {
            println!(
                "Found {} task(s) in {} events:\n",
                task_ids.len(),
                events.len()
            );
            for tid in &task_ids {
                let summary = trace_export::build_replay_summary(&events, tid);
                print_replay_summary(&summary);
                println!();
            }
        }
    }

    Ok(())
}

fn print_replay_summary(summary: &trace_export::ReplaySummary) {
    println!("Task: {}", summary.task_id);
    println!("  Trace:     {}", summary.trace_id);
    println!("  Events:    {}", summary.total_events);
    println!("  Completed: {}", summary.steps_completed);
    println!("  Failed:    {}", summary.steps_failed);
    println!("  Cost:      ${:.4}", summary.total_cost);
    println!("  Latency:   {}ms", summary.total_latency_ms);

    if !summary.events.is_empty() {
        println!("  Steps:");
        for step in &summary.events {
            let agent = if step.agent.is_empty() {
                "-"
            } else {
                &step.agent
            };
            let latency = step
                .latency_ms
                .map(|l| format!("{}ms", l))
                .unwrap_or_default();
            println!(
                "    [{:.19}] {} | {} | {}",
                step.timestamp, step.event_type, agent, latency
            );
        }
    }
}

/// Watch a task's live status (simple poll-and-display).
pub fn watch(task_id: &str, dir: &Path) -> Result<(), String> {
    if let Some(task) = task_model::read_task_state(dir) {
        if task.task_id.starts_with(task_id) || task_id == task.task_id {
            println!("Task: {} [{}]", task.task_id, task.status);
            println!("  Goal:     {}", task.goal);
            println!(
                "  Progress: {}/{} steps",
                task.current_step_index, task.total_steps
            );
            println!("  Errors:   {}", task.consecutive_errors);
            println!("  Updated:  {}", task.updated_at);
            if let Some(ref completed) = task.completed_at {
                println!("  Completed: {}", completed);
            }
            if !task.completed_step_ids.is_empty() {
                println!("  Completed steps:");
                for sid in &task.completed_step_ids {
                    println!("    - {}", &sid[..8.min(sid.len())]);
                }
            }
            return Ok(());
        }
    }

    Err(format!("Task '{}' not found in {}", task_id, dir.display()))
}

/// Validate a data file against a JSON schema (basic structure check).
pub fn validate(schema_path: &Path, data_path: &Path) -> Result<(), String> {
    let schema_str = std::fs::read_to_string(schema_path)
        .map_err(|e| format!("Failed to read schema: {}", e))?;
    let data_str =
        std::fs::read_to_string(data_path).map_err(|e| format!("Failed to read data: {}", e))?;

    // Basic validation: parse both as JSON
    let _schema: serde_json::Value =
        serde_json::from_str(&schema_str).map_err(|e| format!("Invalid schema JSON: {}", e))?;
    let _data: serde_json::Value =
        serde_json::from_str(&data_str).map_err(|e| format!("Invalid data JSON: {}", e))?;

    println!("Schema: {} (valid JSON)", schema_path.display());
    println!("Data:   {} (valid JSON)", data_path.display());
    println!("Basic validation passed.");
    Ok(())
}

/// Compute eval metrics from trace JSONL files.
pub fn eval_cmd(trace_path: &Path, output: Option<&Path>) -> Result<(), String> {
    let report = if trace_path.is_dir() {
        eval::eval_from_dir(trace_path)?
    } else {
        eval::eval_from_file(trace_path)?
    };

    // Print human-readable report
    print!("{}", eval::format_report(&report));

    // Optionally write JSON report
    if let Some(output_path) = output {
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create output directory: {}", e))?;
        }
        let json = serde_json::to_string_pretty(&report)
            .map_err(|e| format!("Failed to serialize report: {}", e))?;
        std::fs::write(output_path, &json).map_err(|e| format!("Failed to write report: {}", e))?;
        println!("\nReport written to: {}", output_path.display());
    }

    Ok(())
}

/// Submit a task for execution.
pub fn submit(
    goal: &str,
    budget: Option<f64>,
    policy: &str,
    max_steps: u32,
    dir: &Path,
) -> Result<(), String> {
    use omnihive_core::tool_protocol::ToolRegistry;
    use omnihive_core::tools::filesystem::FileSystemTool;
    use omnihive_core::tools::shell::{ShellTool, ShellToolConfig};

    let policy_mode = match policy {
        "default" => runner::PolicyMode::Default,
        _ => runner::PolicyMode::Permissive,
    };

    let config = runner::SubmitConfig {
        goal: goal.to_string(),
        budget,
        max_steps,
        policy: policy_mode,
        agents: vec!["default".to_string()],
    };

    // Register available tools
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ShellTool::new(ShellToolConfig {
        allowed_dirs: vec![dir.display().to_string()],
        ..Default::default()
    })));
    registry.register(Box::new(FileSystemTool::new()));

    println!("Submitting task: {}", goal);
    if let Some(b) = budget {
        println!("Budget: ${:.2}", b);
    }
    println!("Max steps: {}", max_steps);
    println!("Policy: {}", policy);
    println!();

    let result = runner::run_task(dir, &config, &registry)?;

    println!("Task: {}", result.task_id);
    println!("Trace: {}", result.trace_id);
    println!("Status: {}", result.status);
    println!("Steps completed: {}", result.steps_completed);
    println!("Total cost: ${:.4}", result.total_cost);
    println!("Trace file: {}", result.trace_file);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omnihive_core::trace_export::{append_trace_event, TraceEvent};
    use std::path::Path;

    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("omnihive_cli_cmd_test_{}", name));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn cleanup(dir: &std::path::PathBuf) {
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Write a minimal trace JSONL with one task_created + task_completed event.
    fn write_trace_jsonl(path: &Path) {
        let e1 = TraceEvent::new("tr-1", "task_created").with_task("t-1");
        let e2 = TraceEvent::new("tr-1", "step_completed")
            .with_task("t-1")
            .with_step("s-1")
            .with_cost(0.01)
            .with_latency(500);
        let e3 = TraceEvent::new("tr-1", "task_completed").with_task("t-1");
        append_trace_event(path, &e1).unwrap();
        append_trace_event(path, &e2).unwrap();
        append_trace_event(path, &e3).unwrap();
    }

    // ── eval_cmd ────────────────────────────────────────────────────────────

    #[test]
    fn test_eval_cmd_with_valid_jsonl_file() {
        let dir = test_dir("eval_file");
        let trace = dir.join("trace.jsonl");
        write_trace_jsonl(&trace);

        let result = eval_cmd(&trace, None);
        assert!(
            result.is_ok(),
            "eval_cmd should succeed with a valid JSONL file"
        );

        cleanup(&dir);
    }

    #[test]
    fn test_eval_cmd_with_directory() {
        let dir = test_dir("eval_dir");
        write_trace_jsonl(&dir.join("trace.jsonl"));

        // eval_cmd should dispatch to eval_from_dir when path is a directory
        let result = eval_cmd(&dir, None);
        assert!(
            result.is_ok(),
            "eval_cmd should succeed with a directory of JSONL files"
        );

        cleanup(&dir);
    }

    #[test]
    fn test_eval_cmd_writes_json_output_file() {
        let dir = test_dir("eval_output");
        let trace = dir.join("trace.jsonl");
        write_trace_jsonl(&trace);

        let output = dir.join("report.json");
        let result = eval_cmd(&trace, Some(&output));
        assert!(result.is_ok());

        // Output JSON file should be created and contain valid JSON
        assert!(output.exists(), "JSON report file should be created");
        let contents = std::fs::read_to_string(&output).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();
        assert!(parsed.get("total_tasks").is_some());
        assert!(parsed.get("success_rate").is_some());

        cleanup(&dir);
    }

    #[test]
    fn test_eval_cmd_output_creates_parent_dirs() {
        let dir = test_dir("eval_output_nested");
        let trace = dir.join("trace.jsonl");
        write_trace_jsonl(&trace);

        // The output path has a nested directory that does not exist yet
        let output = dir.join("reports").join("subdir").join("report.json");
        let result = eval_cmd(&trace, Some(&output));
        assert!(
            result.is_ok(),
            "eval_cmd should create missing parent directories"
        );
        assert!(output.exists());

        cleanup(&dir);
    }

    #[test]
    fn test_eval_cmd_with_empty_jsonl_file() {
        let dir = test_dir("eval_empty");
        let trace = dir.join("empty.jsonl");
        std::fs::write(&trace, "").unwrap();

        // An empty file has zero events → an empty report (not an error)
        let result = eval_cmd(&trace, None);
        assert!(
            result.is_ok(),
            "eval_cmd with empty JSONL should return Ok (empty report)"
        );

        cleanup(&dir);
    }

    #[test]
    fn test_eval_cmd_with_nonexistent_file_returns_err() {
        let dir = test_dir("eval_nonexistent");
        let path = dir.join("trace.jsonl");
        cleanup(&dir); // ensure path does not exist
        let result = eval_cmd(&path, None);
        assert!(
            result.is_err(),
            "eval_cmd should return Err for a nonexistent path"
        );
    }

    #[test]
    fn test_eval_cmd_output_json_has_correct_task_count() {
        let dir = test_dir("eval_task_count");
        let trace = dir.join("trace.jsonl");

        // Write two tasks
        let e1 = TraceEvent::new("tr-1", "task_created").with_task("t-1");
        let e2 = TraceEvent::new("tr-1", "task_completed").with_task("t-1");
        let e3 = TraceEvent::new("tr-2", "task_created").with_task("t-2");
        let e4 = TraceEvent::new("tr-2", "task_failed").with_task("t-2");
        for e in &[e1, e2, e3, e4] {
            append_trace_event(&trace, e).unwrap();
        }

        let output = dir.join("report.json");
        eval_cmd(&trace, Some(&output)).unwrap();

        let contents = std::fs::read_to_string(&output).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(parsed["total_tasks"].as_u64().unwrap(), 2);
        assert_eq!(parsed["successful_tasks"].as_u64().unwrap(), 1);
        assert_eq!(parsed["failed_tasks"].as_u64().unwrap(), 1);

        cleanup(&dir);
    }

    #[test]
    fn test_eval_cmd_directory_skips_non_jsonl_files() {
        let dir = test_dir("eval_skip_non_jsonl");

        // Write a valid JSONL trace
        write_trace_jsonl(&dir.join("events.jsonl"));

        // Write a non-JSONL file that should be skipped
        std::fs::write(dir.join("notes.txt"), "this should be ignored").unwrap();

        let result = eval_cmd(&dir, None);
        assert!(result.is_ok());

        cleanup(&dir);
    }

    // ── validate ────────────────────────────────────────────────────────────

    #[test]
    fn test_validate_with_valid_json_schema_and_data() {
        let dir = test_dir("validate_ok");
        let schema = dir.join("schema.json");
        let data = dir.join("data.json");
        std::fs::write(&schema, r#"{"type":"object"}"#).unwrap();
        std::fs::write(&data, r#"{"name":"Alice"}"#).unwrap();

        let result = validate(&schema, &data);
        assert!(result.is_ok());

        cleanup(&dir);
    }

    #[test]
    fn test_validate_with_invalid_schema_json_returns_err() {
        let dir = test_dir("validate_bad_schema");
        let schema = dir.join("schema.json");
        let data = dir.join("data.json");
        std::fs::write(&schema, "not valid json {{{{").unwrap();
        std::fs::write(&data, r#"{"key":"value"}"#).unwrap();

        let result = validate(&schema, &data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid schema JSON"));

        cleanup(&dir);
    }

    #[test]
    fn test_validate_with_invalid_data_json_returns_err() {
        let dir = test_dir("validate_bad_data");
        let schema = dir.join("schema.json");
        let data = dir.join("data.json");
        std::fs::write(&schema, r#"{"type":"object"}"#).unwrap();
        std::fs::write(&data, "this is not json").unwrap();

        let result = validate(&schema, &data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid data JSON"));

        cleanup(&dir);
    }

    #[test]
    fn test_validate_with_missing_schema_file_returns_err() {
        let dir = test_dir("validate_no_schema");
        let data = dir.join("data.json");
        std::fs::write(&data, r#"{"x":1}"#).unwrap();
        let missing = dir.join("schema_does_not_exist.json");

        let result = validate(&missing, &data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read schema"));

        cleanup(&dir);
    }

    #[test]
    fn test_validate_with_missing_data_file_returns_err() {
        let dir = test_dir("validate_no_data");
        let schema = dir.join("schema.json");
        std::fs::write(&schema, r#"{"type":"object"}"#).unwrap();
        let missing = dir.join("data_does_not_exist.json");

        let result = validate(&schema, &missing);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read data"));

        cleanup(&dir);
    }

    #[test]
    fn test_validate_accepts_any_valid_json_structure() {
        let dir = test_dir("validate_any_json");
        let schema = dir.join("schema.json");
        let data = dir.join("data.json");
        // JSON arrays are valid JSON values too
        std::fs::write(&schema, r#"[1, 2, 3]"#).unwrap();
        std::fs::write(&data, r#"[true, null, "text"]"#).unwrap();

        let result = validate(&schema, &data);
        assert!(
            result.is_ok(),
            "validate should accept any valid JSON, not just objects"
        );

        cleanup(&dir);
    }
}
