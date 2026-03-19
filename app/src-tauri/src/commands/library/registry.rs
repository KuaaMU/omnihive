use std::path::PathBuf;
use crate::models::*;

// ===== Project Registry =====

fn get_registry_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("omnihive")
        .join("projects.json")
}

fn load_registry() -> ProjectRegistry {
    let path = get_registry_path();
    if !path.exists() {
        return ProjectRegistry::default();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

fn save_registry(registry: &ProjectRegistry) -> Result<(), String> {
    let path = get_registry_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create registry dir: {}", e))?;
    }
    let json = serde_json::to_string_pretty(registry)
        .map_err(|e| format!("Serialize error: {}", e))?;
    std::fs::write(&path, &json)
        .map_err(|e| format!("Write error: {}", e))?;
    Ok(())
}

pub fn register_project(name: &str, output_dir: &str) -> Result<(), String> {
    let mut registry = load_registry();
    registry.projects.retain(|p| p.output_dir != output_dir);

    let id = PathBuf::from(output_dir)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    registry.projects.push(ProjectRegistryEntry {
        id,
        name: name.to_string(),
        output_dir: output_dir.to_string(),
        created_at: chrono::Local::now().format("%+").to_string(),
    });

    save_registry(&registry)
}

// ===== Library State Persistence =====

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct LibraryState {
    #[serde(default)]
    pub disabled_personas: Vec<String>,
    #[serde(default)]
    pub disabled_skills: Vec<String>,
    #[serde(default)]
    pub disabled_workflows: Vec<String>,
}

fn get_library_state_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("omnihive")
        .join("library_state.json")
}

pub(crate) fn load_library_state() -> LibraryState {
    let path = get_library_state_path();
    if !path.exists() {
        return LibraryState::default();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

pub(crate) fn save_library_state(state: &LibraryState) -> Result<(), String> {
    let path = get_library_state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create dir: {}", e))?;
    }
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Serialize error: {}", e))?;
    std::fs::write(&path, &json)
        .map_err(|e| format!("Write error: {}", e))?;
    Ok(())
}

// ===== Library Directory Resolution =====

/// Public accessor for library directory resolution (used by runtime skill injection).
pub fn get_library_dir_pub() -> Option<PathBuf> {
    get_library_dir()
}

pub(crate) fn get_library_dir() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let lib = parent.join("library");
            if lib.exists() {
                return Some(lib);
            }
            let dev_lib = parent
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .map(|p| p.join("library"));
            if let Some(ref dl) = dev_lib {
                if dl.exists() {
                    return dev_lib;
                }
            }
        }
    }

    let cwd_lib = PathBuf::from("library");
    if cwd_lib.exists() {
        return Some(cwd_lib);
    }

    // Allow override via environment variable
    if let Ok(env_lib) = std::env::var("OMNIHIVE_LIBRARY_DIR") {
        let env_path = PathBuf::from(&env_lib);
        if env_path.exists() {
            return Some(env_path);
        }
    }

    None
}

// ===== Project CRUD =====

pub(crate) fn list_projects_impl() -> Result<Vec<Project>, String> {
    let registry = load_registry();
    let mut projects = Vec::new();

    for entry in &registry.projects {
        let path = PathBuf::from(&entry.output_dir);
        let config_path = path.join("company.yaml");

        if !config_path.exists() {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = serde_yaml::from_str::<FactoryConfig>(&content) {
                let status = if path.join(".loop.pid").exists() {
                    ProjectStatus::Running
                } else if path.join(".loop.state").exists() {
                    let state =
                        std::fs::read_to_string(path.join(".loop.state")).unwrap_or_default();
                    if state.contains("status=error") {
                        ProjectStatus::Error
                    } else {
                        ProjectStatus::Stopped
                    }
                } else {
                    ProjectStatus::Initializing
                };

                let cycle_count = std::fs::read_to_string(path.join(".cycle_history.json"))
                    .ok()
                    .and_then(|c| serde_json::from_str::<Vec<serde_json::Value>>(&c).ok())
                    .map(|v| v.len() as u32)
                    .unwrap_or(0);

                projects.push(Project {
                    id: entry.id.clone(),
                    name: config.company.name,
                    seed_prompt: config.company.seed_prompt,
                    output_dir: entry.output_dir.clone(),
                    created_at: entry.created_at.clone(),
                    last_active_at: entry.created_at.clone(),
                    status,
                    agent_count: config.org.agents.len(),
                    cycle_count,
                });
            }
        }
    }

    Ok(projects)
}

pub(crate) fn delete_project_impl(id: &str) -> Result<bool, String> {
    let mut registry = load_registry();
    let entry = registry.projects.iter().find(|p| p.id == id).cloned();

    if let Some(entry) = entry {
        let path = PathBuf::from(&entry.output_dir);
        if path.exists() {
            std::fs::remove_dir_all(&path)
                .map_err(|e| format!("Failed to delete: {}", e))?;
        }
        registry.projects.retain(|p| p.id != id);
        save_registry(&registry)?;
        Ok(true)
    } else {
        Ok(false)
    }
}
