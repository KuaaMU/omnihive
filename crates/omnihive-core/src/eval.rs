//! Basic Eval Metrics: compute success rate, first-pass rate, cost per task, P50/P95 latency.
//!
//! Reads trace JSONL files and produces an evaluation report.

use crate::trace_export::{read_trace_events, TraceEvent};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

// ===== Eval Report =====

#[derive(Debug, Clone, Serialize)]
pub struct EvalReport {
    pub total_tasks: usize,
    pub successful_tasks: usize,
    pub failed_tasks: usize,
    pub success_rate: f64,
    pub first_pass_rate: f64,
    pub total_cost: f64,
    pub avg_cost_per_task: f64,
    pub latency_p50_ms: u64,
    pub latency_p95_ms: u64,
    pub total_steps: usize,
    pub total_retries: usize,
    pub tasks: Vec<TaskEval>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskEval {
    pub task_id: String,
    pub status: String,
    pub steps: usize,
    pub retries: usize,
    pub cost: f64,
    pub latency_ms: u64,
    pub first_pass: bool,
}

// ===== Compute Eval =====

/// Compute evaluation metrics from a set of trace events.
pub fn compute_eval(events: &[TraceEvent]) -> EvalReport {
    // Group events by task_id
    let mut task_events: HashMap<String, Vec<&TraceEvent>> = HashMap::new();
    for event in events {
        if let Some(ref tid) = event.task_id {
            task_events.entry(tid.clone()).or_default().push(event);
        }
    }

    let mut tasks = Vec::new();
    let mut all_latencies: Vec<u64> = Vec::new();

    for (task_id, events) in &task_events {
        let eval = evaluate_task(task_id, events);
        all_latencies.push(eval.latency_ms);
        tasks.push(eval);
    }

    tasks.sort_by(|a, b| a.task_id.cmp(&b.task_id));
    all_latencies.sort();

    let total_tasks = tasks.len();
    let successful_tasks = tasks.iter().filter(|t| t.status == "success").count();
    let failed_tasks = tasks.iter().filter(|t| t.status == "failed").count();
    let first_pass_count = tasks.iter().filter(|t| t.first_pass).count();
    let total_cost: f64 = tasks.iter().map(|t| t.cost).sum();
    let total_steps: usize = tasks.iter().map(|t| t.steps).sum();
    let total_retries: usize = tasks.iter().map(|t| t.retries).sum();

    let success_rate = if total_tasks > 0 {
        successful_tasks as f64 / total_tasks as f64
    } else {
        0.0
    };

    let first_pass_rate = if total_tasks > 0 {
        first_pass_count as f64 / total_tasks as f64
    } else {
        0.0
    };

    let avg_cost_per_task = if total_tasks > 0 {
        total_cost / total_tasks as f64
    } else {
        0.0
    };

    let latency_p50 = percentile(&all_latencies, 50);
    let latency_p95 = percentile(&all_latencies, 95);

    EvalReport {
        total_tasks,
        successful_tasks,
        failed_tasks,
        success_rate,
        first_pass_rate,
        total_cost,
        avg_cost_per_task,
        latency_p50_ms: latency_p50,
        latency_p95_ms: latency_p95,
        total_steps,
        total_retries,
        tasks,
    }
}

/// Evaluate a single task from its trace events.
fn evaluate_task(task_id: &str, events: &[&TraceEvent]) -> TaskEval {
    let mut steps = 0usize;
    let mut retries = 0usize;
    let mut cost = 0.0f64;
    let mut latency_ms = 0u64;
    let mut has_success = false;
    let mut has_failure = false;

    for event in events {
        if event.event_type == "step_completed" {
            steps += 1;
        }
        if event.event_type == "step_failed" {
            has_failure = true;
        }
        if event.event_type == "step_retried" {
            retries += 1;
        }
        if event.event_type == "task_completed" {
            has_success = true;
        }
        if event.event_type == "task_failed" {
            has_failure = true;
        }
        if let Some(c) = event.cost {
            cost += c;
        }
        if let Some(l) = event.latency_ms {
            latency_ms += l;
        }
    }

    let status = if has_success {
        "success"
    } else if has_failure {
        "failed"
    } else {
        "in_progress"
    };

    let first_pass = has_success && retries == 0;

    TaskEval {
        task_id: task_id.to_string(),
        status: status.to_string(),
        steps,
        retries,
        cost,
        latency_ms,
        first_pass,
    }
}

/// Compute the Nth percentile from a sorted list.
fn percentile(sorted: &[u64], pct: u32) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((pct as f64 / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    let clamped = idx.min(sorted.len() - 1);
    sorted[clamped]
}

/// Compute eval from a JSONL trace file.
pub fn eval_from_file(path: &Path) -> Result<EvalReport, String> {
    let events = read_trace_events(path)?;
    Ok(compute_eval(&events))
}

/// Compute eval from multiple JSONL trace files in a directory.
pub fn eval_from_dir(dir: &Path) -> Result<EvalReport, String> {
    if !dir.is_dir() {
        return Err(format!("'{}' is not a directory", dir.display()));
    }

    let mut all_events = Vec::new();

    let entries = std::fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            match read_trace_events(&path) {
                Ok(events) => all_events.extend(events),
                Err(e) => {
                    eprintln!("Warning: skipping {}: {}", path.display(), e);
                }
            }
        }
    }

    Ok(compute_eval(&all_events))
}

/// Format an eval report as human-readable text.
pub fn format_report(report: &EvalReport) -> String {
    let mut out = String::new();
    out.push_str("=== Omnihive Eval Report ===\n\n");
    out.push_str(&format!(
        "Tasks:        {} total, {} success, {} failed\n",
        report.total_tasks, report.successful_tasks, report.failed_tasks
    ));
    out.push_str(&format!(
        "Success Rate: {:.1}%\n",
        report.success_rate * 100.0
    ));
    out.push_str(&format!(
        "First-Pass:   {:.1}%\n",
        report.first_pass_rate * 100.0
    ));
    out.push_str(&format!("Total Cost:   ${:.4}\n", report.total_cost));
    out.push_str(&format!(
        "Avg Cost:     ${:.4}/task\n",
        report.avg_cost_per_task
    ));
    out.push_str(&format!("Latency P50:  {}ms\n", report.latency_p50_ms));
    out.push_str(&format!("Latency P95:  {}ms\n", report.latency_p95_ms));
    out.push_str(&format!("Total Steps:  {}\n", report.total_steps));
    out.push_str(&format!("Total Retries:{}\n", report.total_retries));

    if !report.tasks.is_empty() {
        out.push_str("\n--- Per-Task ---\n");
        for task in &report.tasks {
            out.push_str(&format!(
                "  {} | {} | {} steps | {} retries | ${:.4} | {}ms | first_pass={}\n",
                task.task_id,
                task.status,
                task.steps,
                task.retries,
                task.cost,
                task.latency_ms,
                task.first_pass
            ));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace_export::TraceEvent;

    fn make_events() -> Vec<TraceEvent> {
        vec![
            // Task 1: succeeds, no retries
            TraceEvent::new("tr", "task_created").with_task("t-1"),
            TraceEvent::new("tr", "step_completed")
                .with_task("t-1")
                .with_step("s-1")
                .with_cost(0.02)
                .with_latency(3000),
            TraceEvent::new("tr", "step_completed")
                .with_task("t-1")
                .with_step("s-2")
                .with_cost(0.03)
                .with_latency(4000),
            TraceEvent::new("tr", "task_completed").with_task("t-1"),
            // Task 2: fails after retry
            TraceEvent::new("tr", "task_created").with_task("t-2"),
            TraceEvent::new("tr", "step_completed")
                .with_task("t-2")
                .with_step("s-3")
                .with_cost(0.01)
                .with_latency(2000),
            TraceEvent::new("tr", "step_failed")
                .with_task("t-2")
                .with_step("s-4")
                .with_latency(1000),
            TraceEvent::new("tr", "step_retried")
                .with_task("t-2")
                .with_step("s-4"),
            TraceEvent::new("tr", "step_failed")
                .with_task("t-2")
                .with_step("s-4")
                .with_latency(1000),
            TraceEvent::new("tr", "task_failed").with_task("t-2"),
            // Task 3: succeeds after retry (not first-pass)
            TraceEvent::new("tr", "task_created").with_task("t-3"),
            TraceEvent::new("tr", "step_failed")
                .with_task("t-3")
                .with_step("s-5")
                .with_latency(500),
            TraceEvent::new("tr", "step_retried")
                .with_task("t-3")
                .with_step("s-5"),
            TraceEvent::new("tr", "step_completed")
                .with_task("t-3")
                .with_step("s-5")
                .with_cost(0.04)
                .with_latency(2000),
            TraceEvent::new("tr", "task_completed").with_task("t-3"),
        ]
    }

    #[test]
    fn test_compute_eval_totals() {
        let events = make_events();
        let report = compute_eval(&events);
        assert_eq!(report.total_tasks, 3);
        assert_eq!(report.successful_tasks, 2); // t-1, t-3
        assert_eq!(report.failed_tasks, 1); // t-2
    }

    #[test]
    fn test_compute_eval_success_rate() {
        let events = make_events();
        let report = compute_eval(&events);
        assert!((report.success_rate - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_eval_first_pass_rate() {
        let events = make_events();
        let report = compute_eval(&events);
        // Only t-1 is first-pass (no retries and succeeded)
        assert!((report.first_pass_rate - 1.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_eval_cost() {
        let events = make_events();
        let report = compute_eval(&events);
        assert!((report.total_cost - 0.10).abs() < 0.001);
    }

    #[test]
    fn test_compute_eval_retries() {
        let events = make_events();
        let report = compute_eval(&events);
        assert_eq!(report.total_retries, 2); // t-2 has 1 retry, t-3 has 1 retry
    }

    #[test]
    fn test_compute_eval_empty() {
        let report = compute_eval(&[]);
        assert_eq!(report.total_tasks, 0);
        assert_eq!(report.success_rate, 0.0);
        assert_eq!(report.latency_p50_ms, 0);
    }

    #[test]
    fn test_percentile_empty() {
        assert_eq!(percentile(&[], 50), 0);
    }

    #[test]
    fn test_percentile_single() {
        assert_eq!(percentile(&[100], 50), 100);
        assert_eq!(percentile(&[100], 95), 100);
    }

    #[test]
    fn test_percentile_multiple() {
        let sorted = vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];
        let p50 = percentile(&sorted, 50);
        assert!(p50 >= 500 && p50 <= 600);
        let p95 = percentile(&sorted, 95);
        assert!(p95 >= 900 && p95 <= 1000);
    }

    #[test]
    fn test_evaluate_task_success() {
        let events = vec![
            TraceEvent::new("tr", "step_completed")
                .with_task("t-1")
                .with_cost(0.01)
                .with_latency(1000),
            TraceEvent::new("tr", "task_completed").with_task("t-1"),
        ];
        let refs: Vec<&TraceEvent> = events.iter().collect();
        let eval = evaluate_task("t-1", &refs);
        assert_eq!(eval.status, "success");
        assert_eq!(eval.steps, 1);
        assert!(eval.first_pass);
    }

    #[test]
    fn test_evaluate_task_failed() {
        let events = vec![
            TraceEvent::new("tr", "step_failed")
                .with_task("t-1")
                .with_latency(500),
            TraceEvent::new("tr", "task_failed").with_task("t-1"),
        ];
        let refs: Vec<&TraceEvent> = events.iter().collect();
        let eval = evaluate_task("t-1", &refs);
        assert_eq!(eval.status, "failed");
        assert!(!eval.first_pass);
    }

    #[test]
    fn test_evaluate_task_in_progress() {
        let events = vec![
            TraceEvent::new("tr", "task_created").with_task("t-1"),
            TraceEvent::new("tr", "step_completed").with_task("t-1"),
        ];
        let refs: Vec<&TraceEvent> = events.iter().collect();
        let eval = evaluate_task("t-1", &refs);
        assert_eq!(eval.status, "in_progress");
    }

    #[test]
    fn test_format_report() {
        let events = make_events();
        let report = compute_eval(&events);
        let text = format_report(&report);
        assert!(text.contains("Omnihive Eval Report"));
        assert!(text.contains("Success Rate"));
        assert!(text.contains("First-Pass"));
    }

    #[test]
    fn test_eval_from_file() {
        let dir = std::env::temp_dir().join("omnihive_test_eval");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("trace.jsonl");

        // Write test events
        for event in &make_events() {
            crate::trace_export::append_trace_event(&path, event).unwrap();
        }

        let report = eval_from_file(&path).unwrap();
        assert_eq!(report.total_tasks, 3);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_eval_from_dir() {
        let dir = std::env::temp_dir().join("omnihive_test_eval_dir");
        let _ = std::fs::create_dir_all(&dir);

        // Write events to two files
        let path1 = dir.join("trace1.jsonl");
        let path2 = dir.join("trace2.jsonl");

        let e1 = TraceEvent::new("tr1", "task_created").with_task("t-1");
        let e2 = TraceEvent::new("tr1", "task_completed").with_task("t-1");
        crate::trace_export::append_trace_event(&path1, &e1).unwrap();
        crate::trace_export::append_trace_event(&path1, &e2).unwrap();

        let e3 = TraceEvent::new("tr2", "task_created").with_task("t-2");
        let e4 = TraceEvent::new("tr2", "task_failed").with_task("t-2");
        crate::trace_export::append_trace_event(&path2, &e3).unwrap();
        crate::trace_export::append_trace_event(&path2, &e4).unwrap();

        let report = eval_from_dir(&dir).unwrap();
        assert_eq!(report.total_tasks, 2);
        assert_eq!(report.successful_tasks, 1);
        assert_eq!(report.failed_tasks, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
