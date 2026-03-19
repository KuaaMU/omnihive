//! Library commands for personas, skills, workflows, and project management.
//!
//! All `#[tauri::command]` signatures live here and delegate to focused submodules.

pub mod personas;
pub mod registry;
pub mod skills;
pub mod workflows;

use tauri::command;
use crate::models::*;
use registry::LibraryState;

// Re-export for runtime skill injection
pub use registry::get_library_dir_pub;

fn default_lib_fields() -> (bool, Option<String>, Vec<String>) {
    (true, None, vec![])
}

// ===== Tauri Commands =====

#[command]
pub fn list_personas() -> Result<Vec<PersonaInfo>, String> {
    if let Some(p) = personas::load_personas_from_files() {
        return Ok(p);
    }
    Ok(personas::fallback_personas())
}

#[command]
pub fn list_skills() -> Result<Vec<SkillInfo>, String> {
    if let Some(s) = skills::load_skills_from_files() {
        return Ok(s);
    }
    Ok(skills::fallback_skills())
}

#[command]
pub fn list_workflows() -> Result<Vec<WorkflowInfo>, String> {
    if let Some(w) = workflows::load_workflows_from_files() {
        return Ok(w);
    }
    Ok(workflows::fallback_workflows())
}

#[command]
pub fn get_skill_content(skill_id: String) -> Result<String, String> {
    skills::get_skill_content_impl(&skill_id)
}

#[command]
pub fn toggle_library_item(
    item_type: String,
    item_id: String,
    enabled: bool,
) -> Result<bool, String> {
    let state = registry::load_library_state();
    let mut new_state = state;

    match item_type.as_str() {
        "persona" => {
            if enabled {
                new_state.disabled_personas.retain(|id| id != &item_id);
            } else if !new_state.disabled_personas.contains(&item_id) {
                new_state.disabled_personas.push(item_id);
            }
        }
        "skill" => {
            if enabled {
                new_state.disabled_skills.retain(|id| id != &item_id);
            } else if !new_state.disabled_skills.contains(&item_id) {
                new_state.disabled_skills.push(item_id);
            }
        }
        "workflow" => {
            if enabled {
                new_state.disabled_workflows.retain(|id| id != &item_id);
            } else if !new_state.disabled_workflows.contains(&item_id) {
                new_state.disabled_workflows.push(item_id);
            }
        }
        _ => return Err(format!("Unknown item type: {}", item_type)),
    }

    registry::save_library_state(&new_state)?;
    Ok(enabled)
}

#[command]
pub fn get_library_state() -> Result<LibraryState, String> {
    Ok(registry::load_library_state())
}

#[command]
pub fn list_projects() -> Result<Vec<Project>, String> {
    registry::list_projects_impl()
}

#[command]
pub fn get_project(id: String) -> Result<Project, String> {
    let projects = registry::list_projects_impl()?;
    projects
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("Project not found: {}", id))
}

#[command]
pub fn delete_project(id: String) -> Result<bool, String> {
    registry::delete_project_impl(&id)
}

// Re-export register_project for bootstrap usage
pub use registry::register_project;
