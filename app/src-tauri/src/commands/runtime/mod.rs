//! Runtime commands for the autonomous loop engine.
//!
//! This module is the public API surface -- all `#[tauri::command]` signatures live here
//! and delegate to focused submodules.

pub mod credentials;
pub mod cycle_executor;
pub mod loop_manager;
pub mod prompt_builder;
pub mod status;

use std::path::PathBuf;
use std::process::Command;
use tauri::command;
use crate::engine::state;
use crate::models::*;
use credentials::{
    resolve_api_credentials, resolve_runtime_config_impl, load_project_config,
    SelectedProvider, auto_select_provider_internal,
};

// Re-export for use by other modules (e.g. system.rs)
pub use credentials::{resolve_engine_binary, find_binary};

/// Create a Command that suppresses visible console windows on Windows.
#[cfg(target_os = "windows")]
pub(crate) fn silent_command(program: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut cmd = Command::new(program);
    cmd.creation_flags(0x08000000);
    cmd
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn silent_command(program: &str) -> Command {
    Command::new(program)
}

// ===== Tauri Commands =====

#[command]
pub fn start_loop(project_dir: String, engine: String, model: String) -> Result<bool, String> {
    let dir = PathBuf::from(&project_dir);

    if !dir.join("company.yaml").exists() {
        return Err("Not a valid project directory (missing company.yaml)".to_string());
    }

    // Check per-project override first, fall back to global
    let (effective_engine, effective_model) = {
        let override_path = dir.join(".runtime_override.json");
        if override_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&override_path) {
                if let Ok(ovr) = serde_json::from_str::<ProjectRuntimeOverride>(&content) {
                    (
                        ovr.engine.unwrap_or_else(|| engine.clone()),
                        ovr.model.unwrap_or_else(|| model.clone()),
                    )
                } else {
                    (engine.clone(), model.clone())
                }
            } else {
                (engine.clone(), model.clone())
            }
        } else {
            (engine.clone(), model.clone())
        }
    };

    let credentials = resolve_api_credentials(&effective_engine, &effective_model)?;

    let _ = std::fs::create_dir_all(dir.join("logs"));

    state::append_log(
        &dir,
        &format!(
            "Starting loop | Engine: {} | Model: {} | Mode: Direct API ({})",
            engine, model, credentials.api_base_url
        ),
    );

    let config = load_project_config(&dir)?;
    let agent_roles: Vec<String> = config.org.agents.iter().map(|a| a.role.clone()).collect();
    let loop_interval = config.runtime.loop_interval;
    let cycle_timeout = config.runtime.cycle_timeout;
    let max_errors = config.runtime.max_consecutive_errors;

    state::write_state(&dir, "running", 0, 0, 0)?;

    loop_manager::start_loop_impl(
        dir,
        project_dir,
        credentials,
        agent_roles,
        loop_interval,
        cycle_timeout,
        max_errors,
    )
}

#[command]
pub fn stop_loop(project_dir: String) -> Result<bool, String> {
    let dir = PathBuf::from(&project_dir);
    loop_manager::stop_loop_impl(&project_dir, &dir)
}

#[command]
pub fn resolve_runtime_config(
    engine: String,
    model: String,
) -> Result<ResolvedRuntimeConfig, String> {
    resolve_runtime_config_impl(engine, model)
}

#[command]
pub fn get_status(project_dir: String) -> Result<RuntimeStatus, String> {
    status::get_status_impl(&project_dir)
}

#[command]
pub fn get_cycle_history(project_dir: String) -> Result<Vec<CycleResult>, String> {
    status::get_cycle_history_impl(&project_dir)
}

#[command]
pub fn get_agent_memory(project_dir: String, role: String) -> Result<String, String> {
    status::get_agent_memory_impl(&project_dir, &role)
}

#[command]
pub fn get_handoff_note(project_dir: String) -> Result<String, String> {
    status::get_handoff_note_impl(&project_dir)
}

#[command]
pub fn tail_log(project_dir: String, lines: usize) -> Result<Vec<String>, String> {
    status::tail_log_impl(&project_dir, lines)
}

#[command]
pub fn test_api_call(engine: String, model: String, message: String) -> Result<String, String> {
    status::test_api_call_impl(&engine, &model, &message)
}

#[command]
pub fn get_project_runtime_override(
    project_dir: String,
) -> Result<ProjectRuntimeOverride, String> {
    status::get_project_runtime_override_impl(&project_dir)
}

#[command]
pub fn set_project_runtime_override(
    project_dir: String,
    config: ProjectRuntimeOverride,
) -> Result<bool, String> {
    status::set_project_runtime_override_impl(&project_dir, &config)
}

#[command]
pub fn get_project_events(
    project_dir: String,
    limit: Option<usize>,
) -> Result<Vec<ProjectEvent>, String> {
    status::get_project_events_impl(&project_dir, limit)
}

#[command]
pub fn auto_select_provider() -> Result<SelectedProvider, String> {
    let (_, selected) = auto_select_provider_internal()?;
    Ok(selected)
}
