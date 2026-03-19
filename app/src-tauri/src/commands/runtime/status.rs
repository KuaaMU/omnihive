use super::credentials::resolve_api_credentials;
use super::cycle_executor::load_cycle_history;
use super::loop_manager::RUNNING_LOOPS;
use crate::engine::api_client;
use crate::engine::state;
use crate::models::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

// ===== Project Events =====

pub(crate) static PROJECT_EVENTS: std::sync::LazyLock<Mutex<HashMap<String, Vec<ProjectEvent>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn emit_project_event(
    project_dir: &str,
    event_type: &str,
    agent: &str,
    summary: &str,
    details: &str,
) {
    let event = ProjectEvent {
        id: format!("{}-{}", chrono::Local::now().timestamp_millis(), agent),
        timestamp: chrono::Local::now().format("%+").to_string(),
        event_type: event_type.to_string(),
        agent: agent.to_string(),
        summary: summary.to_string(),
        details: details.to_string(),
    };

    if let Ok(mut map) = PROJECT_EVENTS.lock() {
        let events = map.entry(project_dir.to_string()).or_default();
        events.push(event);
        if events.len() > 200 {
            let drain_count = events.len() - 200;
            events.drain(..drain_count);
        }
    }
}

// ===== Status Queries =====

pub(crate) fn get_status_impl(project_dir: &str) -> Result<RuntimeStatus, String> {
    let dir = PathBuf::from(project_dir);
    let state_file = dir.join(".loop.state");

    let is_running = {
        let loops = RUNNING_LOOPS.lock().map_err(|e| e.to_string())?;
        loops
            .get(project_dir)
            .map(|flag| !flag.load(Ordering::Relaxed))
            .unwrap_or(false)
    };

    let (current_cycle, total_cycles, consecutive_errors, last_cycle_at) =
        state::parse_state_file(&state_file);

    // Clean up stale "running" state
    if !is_running {
        if let Ok(content) = std::fs::read_to_string(&state_file) {
            if content.contains("status=running") {
                state::write_state(
                    &dir,
                    "stopped",
                    current_cycle,
                    total_cycles,
                    consecutive_errors,
                )
                .ok();
            }
        }
        if let Ok(mut loops) = RUNNING_LOOPS.lock() {
            if let Some(flag) = loops.get(project_dir) {
                if flag.load(Ordering::Relaxed) {
                    loops.remove(project_dir);
                }
            }
        }
    }

    Ok(RuntimeStatus {
        is_running,
        pid: None,
        current_cycle,
        total_cycles,
        consecutive_errors,
        last_cycle_at,
        uptime_seconds: 0,
    })
}

pub(crate) fn get_cycle_history_impl(project_dir: &str) -> Result<Vec<CycleResult>, String> {
    let dir = PathBuf::from(project_dir);
    Ok(load_cycle_history(&dir))
}

pub(crate) fn get_agent_memory_impl(project_dir: &str, role: &str) -> Result<String, String> {
    let dir = PathBuf::from(project_dir);
    let memory_path = dir.join(format!("memories/agents/{}/MEMORY.md", role));
    if !memory_path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&memory_path).map_err(|e| format!("Failed to read agent memory: {}", e))
}

pub(crate) fn get_handoff_note_impl(project_dir: &str) -> Result<String, String> {
    let dir = PathBuf::from(project_dir);
    let handoff_path = dir.join("memories/HANDOFF.md");
    if !handoff_path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&handoff_path)
        .map_err(|e| format!("Failed to read handoff note: {}", e))
}

pub(crate) fn tail_log_impl(project_dir: &str, lines: usize) -> Result<Vec<String>, String> {
    let dir = PathBuf::from(project_dir);
    let log_file = dir.join("logs/auto-loop.log");

    if !log_file.exists() {
        return Ok(vec!["No log file yet. Start the loop to begin.".to_string()]);
    }

    let content =
        std::fs::read_to_string(&log_file).map_err(|e| format!("Failed to read log: {}", e))?;

    if content.is_empty() {
        return Ok(vec![
            "Log file is empty. Waiting for activity...".to_string()
        ]);
    }

    let all_lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let start = if all_lines.len() > lines {
        all_lines.len() - lines
    } else {
        0
    };

    Ok(all_lines[start..].to_vec())
}

// ===== Test API Call =====

pub(crate) fn test_api_call_impl(
    engine: &str,
    model: &str,
    message: &str,
) -> Result<String, String> {
    let credentials = resolve_api_credentials(engine, model)?;

    let api_config = api_client::ApiCallConfig {
        api_key: credentials.api_key,
        api_base_url: credentials.api_base_url,
        model: credentials.model,
        system_prompt: "You are a helpful assistant. Reply concisely.".to_string(),
        user_message: if message.is_empty() {
            "Say hello in one sentence.".to_string()
        } else {
            message.to_string()
        },
        timeout_secs: 30,
        anthropic_version: credentials.anthropic_version,
        extra_headers: credentials.extra_headers,
        force_stream: credentials.force_stream,
        api_format: if credentials.engine_type == "openai" {
            "openai".to_string()
        } else {
            credentials.api_format
        },
    };

    let response = api_client::call_api(&api_config)?;
    Ok(format!(
        "[{}in/{}out] {}",
        response.input_tokens, response.output_tokens, response.text
    ))
}

// ===== Runtime Override =====

pub(crate) fn get_project_runtime_override_impl(
    project_dir: &str,
) -> Result<ProjectRuntimeOverride, String> {
    let dir = PathBuf::from(project_dir);
    let override_path = dir.join(".runtime_override.json");
    if !override_path.exists() {
        return Ok(ProjectRuntimeOverride::default());
    }
    let content = std::fs::read_to_string(&override_path)
        .map_err(|e| format!("Failed to read runtime override: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse runtime override: {}", e))
}

pub(crate) fn set_project_runtime_override_impl(
    project_dir: &str,
    config: &ProjectRuntimeOverride,
) -> Result<bool, String> {
    let dir = PathBuf::from(project_dir);
    let override_path = dir.join(".runtime_override.json");
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize override: {}", e))?;
    std::fs::write(&override_path, json).map_err(|e| format!("Failed to write override: {}", e))?;
    Ok(true)
}

// ===== Project Events Query =====

pub(crate) fn get_project_events_impl(
    project_dir: &str,
    limit: Option<usize>,
) -> Result<Vec<ProjectEvent>, String> {
    let max = limit.unwrap_or(50);
    if let Ok(map) = PROJECT_EVENTS.lock() {
        if let Some(events) = map.get(project_dir) {
            let start = if events.len() > max {
                events.len() - max
            } else {
                0
            };
            return Ok(events[start..].to_vec());
        }
    }
    Ok(Vec::new())
}
