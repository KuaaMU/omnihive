use crate::models::*;
use std::fs;
use std::path::PathBuf;
use tauri::command;

// ===== Skill Scanning =====

/// Known skill directories to scan
fn get_skill_scan_dirs() -> Vec<(String, PathBuf)> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut dirs = vec![
        ("claude".to_string(), home.join(".claude").join("skills")),
        ("codex".to_string(), home.join(".codex").join("skills")),
        ("gemini".to_string(), home.join(".gemini").join("skills")),
        (
            "opencode".to_string(),
            home.join(".config").join("opencode").join("skills"),
        ),
        (
            "openclaw".to_string(),
            home.join(".openclaw").join("skills"),
        ),
    ];

    // Also scan our own library
    if let Ok(exe_dir) = std::env::current_exe() {
        if let Some(parent) = exe_dir.parent() {
            let lib_skills = parent.join("library").join("real-skills");
            if lib_skills.exists() {
                dirs.push(("omnihive".to_string(), lib_skills));
            }
        }
    }

    dirs
}

/// Parse SKILL.md to extract name and description
fn parse_skill_md(content: &str) -> (String, String) {
    let mut name = String::new();
    let mut description = String::new();
    let mut found_header = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if !found_header {
            if let Some(h) = trimmed.strip_prefix("# ") {
                name = h.trim().to_string();
                found_header = true;
            }
        } else if !trimmed.is_empty() && description.is_empty() {
            // First non-empty line after header = description
            description = trimmed.to_string();
        }
        if !name.is_empty() && !description.is_empty() {
            break;
        }
    }

    (name, description)
}

/// Parse agent markdown file for metadata
fn parse_agent_md(content: &str) -> (String, String, String, Vec<String>) {
    let mut name = String::new();
    let mut role = String::new();
    let mut expertise = String::new();
    let mut capabilities: Vec<String> = Vec::new();

    let mut in_capabilities = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if let Some(h) = trimmed.strip_prefix("# ") {
            name = h.trim().to_string();
            continue;
        }

        if trimmed.starts_with("**Role:**") || trimmed.starts_with("Role:") {
            role = trimmed
                .trim_start_matches("**Role:**")
                .trim_start_matches("Role:")
                .trim()
                .trim_start_matches("**")
                .trim_end_matches("**")
                .trim()
                .to_string();
            continue;
        }

        if trimmed.starts_with("**Expertise:**") || trimmed.starts_with("Expertise:") {
            expertise = trimmed
                .trim_start_matches("**Expertise:**")
                .trim_start_matches("Expertise:")
                .trim()
                .to_string();
            continue;
        }

        if trimmed.contains("Capabilities") || trimmed.contains("capabilities") {
            in_capabilities = true;
            continue;
        }

        if in_capabilities {
            if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                let cap = trimmed
                    .trim_start_matches("- ")
                    .trim_start_matches("* ")
                    .to_string();
                if capabilities.len() < 8 {
                    capabilities.push(cap);
                }
            } else if trimmed.starts_with('#') || (trimmed.is_empty() && !capabilities.is_empty()) {
                in_capabilities = false;
            }
        }
    }

    (name, role, expertise, capabilities)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScannedSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,    // "claude", "codex", "gemini", etc.
    pub directory: String, // folder name
    pub full_path: String, // absolute path
    pub has_skill_md: bool,
}

#[command]
pub fn scan_local_skills() -> Result<Vec<ScannedSkill>, String> {
    let mut results = Vec::new();
    let scan_dirs = get_skill_scan_dirs();

    for (source, dir) in &scan_dirs {
        if !dir.exists() {
            continue;
        }

        let entries =
            fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {}", dir.display(), e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let dir_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            let skill_md = path.join("SKILL.md");
            let has_skill_md = skill_md.exists();

            let (name, description) = if has_skill_md {
                let content = fs::read_to_string(&skill_md).unwrap_or_default();
                let (n, d) = parse_skill_md(&content);
                (if n.is_empty() { dir_name.clone() } else { n }, d)
            } else {
                (dir_name.clone(), String::new())
            };

            let id = format!("{}:{}", source, dir_name);

            results.push(ScannedSkill {
                id,
                name,
                description,
                source: source.clone(),
                directory: dir_name,
                full_path: path.display().to_string(),
                has_skill_md,
            });
        }
    }

    results.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(results)
}

// ===== Custom Skill Management =====

fn get_custom_skills_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("omnihive")
        .join("custom-skills")
}

fn get_custom_agents_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("omnihive")
        .join("custom-agents")
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AddSkillRequest {
    pub name: String,
    pub description: String,
    pub content: String,
    pub category: String,
}

#[command]
pub fn add_custom_skill(skill: AddSkillRequest) -> Result<SkillInfo, String> {
    let dir = get_custom_skills_dir();
    let slug = skill
        .name
        .to_lowercase()
        .replace(' ', "-")
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "");

    let skill_dir = dir.join(&slug);
    fs::create_dir_all(&skill_dir).map_err(|e| format!("Failed to create skill dir: {}", e))?;

    // Write SKILL.md
    let skill_md_content = format!(
        "# {}\n\n{}\n\n## Category\n\n{}\n\n## Content\n\n{}",
        skill.name, skill.description, skill.category, skill.content
    );
    fs::write(skill_dir.join("SKILL.md"), &skill_md_content)
        .map_err(|e| format!("Failed to write SKILL.md: {}", e))?;

    Ok(SkillInfo {
        id: format!("custom:{}", slug),
        name: skill.name,
        category: skill.category,
        description: skill.description,
        source: "custom".to_string(),
        content_preview: skill.content.chars().take(200).collect(),
        enabled: true,
        file_path: Some(skill_dir.display().to_string()),
        tags: vec![],
    })
}

#[command]
pub fn remove_custom_skill(skill_id: String) -> Result<bool, String> {
    let slug = skill_id.strip_prefix("custom:").unwrap_or(&skill_id);
    let skill_dir = get_custom_skills_dir().join(slug);

    if skill_dir.exists() {
        fs::remove_dir_all(&skill_dir).map_err(|e| format!("Failed to remove skill: {}", e))?;
    }

    Ok(true)
}

// ===== Custom Agent Management =====

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AddAgentRequest {
    pub name: String,
    pub role: String,
    pub expertise: String,
    pub mental_models: Vec<String>,
    pub core_capabilities: Vec<String>,
    pub layer: String,
}

#[command]
pub fn add_custom_agent(agent: AddAgentRequest) -> Result<PersonaInfo, String> {
    let dir = get_custom_agents_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create agents dir: {}", e))?;

    let slug = format!(
        "{}-{}",
        agent.role.to_lowercase().replace(' ', "-"),
        agent.name.to_lowercase().replace(' ', "-")
    )
    .replace(|c: char| !c.is_alphanumeric() && c != '-', "");

    let file_path = dir.join(format!("{}.md", slug));

    let mental_models_str = agent
        .mental_models
        .iter()
        .map(|m| format!("- {}", m))
        .collect::<Vec<_>>()
        .join("\n");

    let capabilities_str = agent
        .core_capabilities
        .iter()
        .map(|c| format!("- {}", c))
        .collect::<Vec<_>>()
        .join("\n");

    let content = format!(
        "# {name}\n\n\
        **Role:** {role}\n\n\
        **Expertise:** {expertise}\n\n\
        **Layer:** {layer}\n\n\
        ## Mental Models\n\n\
        {mental_models}\n\n\
        ## Core Capabilities\n\n\
        {capabilities}\n",
        name = agent.name,
        role = agent.role,
        expertise = agent.expertise,
        layer = agent.layer,
        mental_models = mental_models_str,
        capabilities = capabilities_str,
    );

    fs::write(&file_path, &content).map_err(|e| format!("Failed to write agent: {}", e))?;

    Ok(PersonaInfo {
        id: format!("custom:{}", slug),
        name: agent.name,
        role: agent.role,
        expertise: agent.expertise,
        mental_models: agent.mental_models,
        core_capabilities: agent.core_capabilities,
        enabled: true,
        file_path: Some(file_path.display().to_string()),
        tags: vec![agent.layer],
    })
}

#[command]
pub fn remove_custom_agent(agent_id: String) -> Result<bool, String> {
    let slug = agent_id.strip_prefix("custom:").unwrap_or(&agent_id);
    let file_path = get_custom_agents_dir().join(format!("{}.md", slug));

    if file_path.exists() {
        fs::remove_file(&file_path).map_err(|e| format!("Failed to remove agent: {}", e))?;
    }

    Ok(true)
}

/// List custom agents from disk
#[command]
pub fn list_custom_agents() -> Result<Vec<PersonaInfo>, String> {
    let dir = get_custom_agents_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut results = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("Failed to read custom agents: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            let content = fs::read_to_string(&path).unwrap_or_default();
            let (name, role, expertise, capabilities) = parse_agent_md(&content);

            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            if !name.is_empty() {
                results.push(PersonaInfo {
                    id: format!("custom:{}", stem),
                    name,
                    role,
                    expertise,
                    mental_models: vec![],
                    core_capabilities: capabilities,
                    enabled: true,
                    file_path: Some(path.display().to_string()),
                    tags: vec!["custom".to_string()],
                });
            }
        }
    }

    Ok(results)
}

/// List custom skills from disk
#[command]
pub fn list_custom_skills() -> Result<Vec<SkillInfo>, String> {
    let dir = get_custom_skills_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut results = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("Failed to read custom skills: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }

        let content = fs::read_to_string(&skill_md).unwrap_or_default();
        let (name, description) = parse_skill_md(&content);

        results.push(SkillInfo {
            id: format!("custom:{}", dir_name),
            name: if name.is_empty() {
                dir_name.clone()
            } else {
                name
            },
            category: "custom".to_string(),
            description,
            source: "custom".to_string(),
            content_preview: content.chars().take(200).collect(),
            enabled: true,
            file_path: Some(path.display().to_string()),
            tags: vec![],
        });
    }

    Ok(results)
}

// ===== Custom Workflow Management =====

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AddWorkflowRequest {
    pub name: String,
    pub description: String,
    pub chain: Vec<String>,
    pub convergence_cycles: u32,
}

fn get_custom_workflows_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("omnihive")
        .join("custom_workflows.json")
}

fn load_custom_workflows_file() -> Vec<WorkflowInfo> {
    let path = get_custom_workflows_path();
    if !path.exists() {
        return vec![];
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

fn save_custom_workflows_file(workflows: &[WorkflowInfo]) -> Result<(), String> {
    let path = get_custom_workflows_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create dir: {}", e))?;
    }
    let json =
        serde_json::to_string_pretty(workflows).map_err(|e| format!("Serialize error: {}", e))?;
    fs::write(&path, &json).map_err(|e| format!("Write error: {}", e))?;
    Ok(())
}

#[command]
pub fn add_custom_workflow(workflow: AddWorkflowRequest) -> Result<WorkflowInfo, String> {
    let slug = workflow
        .name
        .to_lowercase()
        .replace(' ', "-")
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "");

    let new_workflow = WorkflowInfo {
        id: format!("custom:{}", slug),
        name: workflow.name,
        description: workflow.description,
        chain: workflow.chain,
        convergence_cycles: workflow.convergence_cycles,
        enabled: true,
        file_path: None,
        tags: vec!["custom".to_string()],
    };

    let mut all = load_custom_workflows_file();
    // Avoid duplicates
    all.retain(|w| w.id != new_workflow.id);
    all.push(new_workflow.clone());
    save_custom_workflows_file(&all)?;

    Ok(new_workflow)
}

#[command]
pub fn remove_custom_workflow(workflow_id: String) -> Result<bool, String> {
    let mut all = load_custom_workflows_file();
    let before = all.len();
    all.retain(|w| w.id != workflow_id);

    if all.len() < before {
        save_custom_workflows_file(&all)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[command]
pub fn list_custom_workflows() -> Result<Vec<WorkflowInfo>, String> {
    Ok(load_custom_workflows_file())
}

// ===== Update Operations =====

#[command]
pub fn update_custom_agent(
    agent_id: String,
    agent: AddAgentRequest,
) -> Result<PersonaInfo, String> {
    let slug = agent_id.strip_prefix("custom:").unwrap_or(&agent_id);
    let dir = get_custom_agents_dir();
    let file_path = dir.join(format!("{}.md", slug));

    if !file_path.exists() {
        return Err(format!("Agent not found: {}", agent_id));
    }

    let mental_models_str = agent
        .mental_models
        .iter()
        .map(|m| format!("- {}", m))
        .collect::<Vec<_>>()
        .join("\n");

    let capabilities_str = agent
        .core_capabilities
        .iter()
        .map(|c| format!("- {}", c))
        .collect::<Vec<_>>()
        .join("\n");

    let content = format!(
        "# {name}\n\n\
        **Role:** {role}\n\n\
        **Expertise:** {expertise}\n\n\
        **Layer:** {layer}\n\n\
        ## Mental Models\n\n\
        {mental_models}\n\n\
        ## Core Capabilities\n\n\
        {capabilities}\n",
        name = agent.name,
        role = agent.role,
        expertise = agent.expertise,
        layer = agent.layer,
        mental_models = mental_models_str,
        capabilities = capabilities_str,
    );

    fs::write(&file_path, &content).map_err(|e| format!("Failed to write agent: {}", e))?;

    Ok(PersonaInfo {
        id: format!("custom:{}", slug),
        name: agent.name,
        role: agent.role,
        expertise: agent.expertise,
        mental_models: agent.mental_models,
        core_capabilities: agent.core_capabilities,
        enabled: true,
        file_path: Some(file_path.display().to_string()),
        tags: vec![agent.layer],
    })
}

#[command]
pub fn update_custom_skill(skill_id: String, skill: AddSkillRequest) -> Result<SkillInfo, String> {
    let slug = skill_id.strip_prefix("custom:").unwrap_or(&skill_id);
    let dir = get_custom_skills_dir();
    let skill_dir = dir.join(slug);

    if !skill_dir.exists() {
        return Err(format!("Skill not found: {}", skill_id));
    }

    let skill_md_content = format!(
        "# {}\n\n{}\n\n## Category\n\n{}\n\n## Content\n\n{}",
        skill.name, skill.description, skill.category, skill.content
    );
    fs::write(skill_dir.join("SKILL.md"), &skill_md_content)
        .map_err(|e| format!("Failed to write SKILL.md: {}", e))?;

    Ok(SkillInfo {
        id: format!("custom:{}", slug),
        name: skill.name,
        category: skill.category,
        description: skill.description,
        source: "custom".to_string(),
        content_preview: skill.content.chars().take(200).collect(),
        enabled: true,
        file_path: Some(skill_dir.display().to_string()),
        tags: vec![],
    })
}

#[command]
pub fn update_custom_workflow(
    workflow_id: String,
    workflow: AddWorkflowRequest,
) -> Result<WorkflowInfo, String> {
    let mut all = load_custom_workflows_file();
    let idx = all.iter().position(|w| w.id == workflow_id);

    match idx {
        Some(i) => {
            let updated = WorkflowInfo {
                id: workflow_id,
                name: workflow.name,
                description: workflow.description,
                chain: workflow.chain,
                convergence_cycles: workflow.convergence_cycles,
                enabled: all[i].enabled,
                file_path: None,
                tags: vec!["custom".to_string()],
            };
            all[i] = updated.clone();
            save_custom_workflows_file(&all)?;
            Ok(updated)
        }
        None => Err(format!("Workflow not found: {}", workflow_id)),
    }
}
