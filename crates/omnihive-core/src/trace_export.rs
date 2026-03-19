//! JSONL trace export and replay.
//!
//! Writes trace events as newline-delimited JSON matching trace.schema.json.
//! Provides a replay parser that reconstructs step sequences from JSONL.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::Path;

// ===== Trace Event =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub trace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    pub event_type: String,
    pub timestamp: String,
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

impl TraceEvent {
    pub fn new(trace_id: &str, event_type: &str) -> Self {
        Self {
            trace_id: trace_id.to_string(),
            task_id: None,
            step_id: None,
            event_type: event_type.to_string(),
            timestamp: chrono::Local::now().format("%+").to_string(),
            payload: serde_json::Value::Null,
            cost: None,
            latency_ms: None,
        }
    }

    pub fn with_task(mut self, task_id: &str) -> Self {
        self.task_id = Some(task_id.to_string());
        self
    }

    pub fn with_step(mut self, step_id: &str) -> Self {
        self.step_id = Some(step_id.to_string());
        self
    }

    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    pub fn with_cost(mut self, cost: f64) -> Self {
        self.cost = Some(cost);
        self
    }

    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }
}

// ===== JSONL Writer =====

/// Append a trace event as a single JSON line to a file.
pub fn append_trace_event(path: &Path, event: &TraceEvent) -> Result<(), String> {
    let line = serde_json::to_string(event)
        .map_err(|e| format!("Failed to serialize trace event: {}", e))?;

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("Failed to open trace file: {}", e))?;

    writeln!(file, "{}", line)
        .map_err(|e| format!("Failed to write trace event: {}", e))
}

// ===== JSONL Reader =====

/// Read all trace events from a JSONL file.
pub fn read_trace_events(path: &Path) -> Result<Vec<TraceEvent>, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("Failed to open trace file: {}", e))?;
    let reader = std::io::BufReader::new(file);
    let mut events = Vec::new();

    for (line_num, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("Read error at line {}: {}", line_num + 1, e))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let event: TraceEvent = serde_json::from_str(trimmed)
            .map_err(|e| format!("Parse error at line {}: {}", line_num + 1, e))?;
        events.push(event);
    }

    Ok(events)
}

/// Filter trace events by task_id.
pub fn filter_by_task(events: &[TraceEvent], task_id: &str) -> Vec<TraceEvent> {
    events
        .iter()
        .filter(|e| e.task_id.as_deref() == Some(task_id))
        .cloned()
        .collect()
}

// ===== Replay Summary =====

#[derive(Debug, Clone, Serialize)]
pub struct ReplaySummary {
    pub task_id: String,
    pub trace_id: String,
    pub total_events: usize,
    pub steps_completed: usize,
    pub steps_failed: usize,
    pub total_cost: f64,
    pub total_latency_ms: u64,
    pub events: Vec<ReplayStep>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayStep {
    pub step_id: String,
    pub agent: String,
    pub event_type: String,
    pub timestamp: String,
    pub latency_ms: Option<u64>,
    pub cost: Option<f64>,
}

/// Build a replay summary from trace events for a specific task.
pub fn build_replay_summary(events: &[TraceEvent], task_id: &str) -> ReplaySummary {
    let task_events = filter_by_task(events, task_id);

    let trace_id = task_events
        .first()
        .map(|e| e.trace_id.clone())
        .unwrap_or_default();

    let mut steps_completed = 0usize;
    let mut steps_failed = 0usize;
    let mut total_cost = 0.0f64;
    let mut total_latency_ms = 0u64;
    let mut replay_steps = Vec::new();

    for event in &task_events {
        if event.event_type == "step_completed" {
            steps_completed += 1;
        }
        if event.event_type == "step_failed" {
            steps_failed += 1;
        }
        if let Some(cost) = event.cost {
            total_cost += cost;
        }
        if let Some(latency) = event.latency_ms {
            total_latency_ms += latency;
        }

        let agent = event
            .payload
            .get("agent")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        replay_steps.push(ReplayStep {
            step_id: event.step_id.clone().unwrap_or_default(),
            agent,
            event_type: event.event_type.clone(),
            timestamp: event.timestamp.clone(),
            latency_ms: event.latency_ms,
            cost: event.cost,
        });
    }

    ReplaySummary {
        task_id: task_id.to_string(),
        trace_id,
        total_events: task_events.len(),
        steps_completed,
        steps_failed,
        total_cost,
        total_latency_ms,
        events: replay_steps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_event_new() {
        let e = TraceEvent::new("tr-1", "step_completed");
        assert_eq!(e.trace_id, "tr-1");
        assert_eq!(e.event_type, "step_completed");
        assert!(e.task_id.is_none());
    }

    #[test]
    fn test_trace_event_builder() {
        let e = TraceEvent::new("tr-1", "step_completed")
            .with_task("t-1")
            .with_step("s-1")
            .with_cost(0.05)
            .with_latency(3000)
            .with_payload(serde_json::json!({"agent": "ceo", "tokens": 500}));

        assert_eq!(e.task_id, Some("t-1".to_string()));
        assert_eq!(e.step_id, Some("s-1".to_string()));
        assert_eq!(e.cost, Some(0.05));
        assert_eq!(e.latency_ms, Some(3000));
        assert_eq!(e.payload["agent"], "ceo");
    }

    #[test]
    fn test_trace_event_serde_roundtrip() {
        let e = TraceEvent::new("tr", "api_call")
            .with_task("t")
            .with_cost(0.01);
        let json = serde_json::to_string(&e).unwrap();
        let parsed: TraceEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.trace_id, "tr");
        assert_eq!(parsed.cost, Some(0.01));
    }

    #[test]
    fn test_write_and_read_jsonl() {
        let dir = std::env::temp_dir().join("omnihive_test_trace_jsonl");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("trace.jsonl");
        let _ = std::fs::remove_file(&path);

        let e1 = TraceEvent::new("tr-1", "task_created").with_task("t-1");
        let e2 = TraceEvent::new("tr-1", "step_completed")
            .with_task("t-1")
            .with_step("s-1")
            .with_cost(0.03)
            .with_latency(5000);
        let e3 = TraceEvent::new("tr-1", "step_failed")
            .with_task("t-1")
            .with_step("s-2");

        append_trace_event(&path, &e1).unwrap();
        append_trace_event(&path, &e2).unwrap();
        append_trace_event(&path, &e3).unwrap();

        let events = read_trace_events(&path).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type, "task_created");
        assert_eq!(events[1].event_type, "step_completed");
        assert_eq!(events[2].event_type, "step_failed");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_filter_by_task() {
        let events = vec![
            TraceEvent::new("tr", "a").with_task("t-1"),
            TraceEvent::new("tr", "b").with_task("t-2"),
            TraceEvent::new("tr", "c").with_task("t-1"),
        ];
        let filtered = filter_by_task(&events, "t-1");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_build_replay_summary() {
        let events = vec![
            TraceEvent::new("tr", "task_created").with_task("t-1"),
            TraceEvent::new("tr", "step_completed")
                .with_task("t-1")
                .with_step("s-1")
                .with_cost(0.02)
                .with_latency(3000)
                .with_payload(serde_json::json!({"agent": "ceo"})),
            TraceEvent::new("tr", "step_completed")
                .with_task("t-1")
                .with_step("s-2")
                .with_cost(0.03)
                .with_latency(4000)
                .with_payload(serde_json::json!({"agent": "devops"})),
            TraceEvent::new("tr", "step_failed")
                .with_task("t-1")
                .with_step("s-3")
                .with_latency(1000),
        ];

        let summary = build_replay_summary(&events, "t-1");
        assert_eq!(summary.task_id, "t-1");
        assert_eq!(summary.total_events, 4);
        assert_eq!(summary.steps_completed, 2);
        assert_eq!(summary.steps_failed, 1);
        assert!((summary.total_cost - 0.05).abs() < 0.001);
        assert_eq!(summary.total_latency_ms, 8000);
        assert_eq!(summary.events.len(), 4);
        assert_eq!(summary.events[1].agent, "ceo");
    }

    #[test]
    fn test_replay_summary_empty_task() {
        let summary = build_replay_summary(&[], "nonexistent");
        assert_eq!(summary.total_events, 0);
        assert_eq!(summary.steps_completed, 0);
        assert_eq!(summary.total_cost, 0.0);
    }
}
