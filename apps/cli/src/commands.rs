use std::path::Path;
use omnihive_core::eval;
use omnihive_core::task_model;
use omnihive_core::trace_export;

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
        let content = std::fs::read_to_string(&legacy)
            .map_err(|e| format!("Failed to read state: {}", e))?;
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
        let mut task_ids: Vec<String> = events
            .iter()
            .filter_map(|e| e.task_id.clone())
            .collect();
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
            println!("Found {} task(s) in {} events:\n", task_ids.len(), events.len());
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
            let agent = if step.agent.is_empty() { "-" } else { &step.agent };
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
        if task.task_id.starts_with(task_id) || task_id == &task.task_id {
            println!("Task: {} [{}]", task.task_id, task.status);
            println!("  Goal:     {}", task.goal);
            println!("  Progress: {}/{} steps", task.current_step_index, task.total_steps);
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
    let data_str = std::fs::read_to_string(data_path)
        .map_err(|e| format!("Failed to read data: {}", e))?;

    // Basic validation: parse both as JSON
    let _schema: serde_json::Value = serde_json::from_str(&schema_str)
        .map_err(|e| format!("Invalid schema JSON: {}", e))?;
    let _data: serde_json::Value = serde_json::from_str(&data_str)
        .map_err(|e| format!("Invalid data JSON: {}", e))?;

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
        std::fs::write(output_path, &json)
            .map_err(|e| format!("Failed to write report: {}", e))?;
        println!("\nReport written to: {}", output_path.display());
    }

    Ok(())
}
