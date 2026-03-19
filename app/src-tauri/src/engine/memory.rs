use crate::models::*;
use std::fs;
use std::path::Path;

pub fn read_consensus(project_dir: &Path) -> Result<ConsensusState, String> {
    let path = project_dir.join("memories/consensus.md");
    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read consensus: {}", e))?;

    // Parse the markdown content
    let mut company_name = String::new();
    let mut mission = String::new();
    let mut status = ProjectStatus::Initializing;
    let mut cycle: u32 = 0;
    let mut revenue = String::from("$0");
    let mut current_focus = String::new();
    let mut next_action = String::new();
    let active_projects: Vec<String> = Vec::new();

    let mut in_focus = false;
    let mut in_next = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("- **Company**:") {
            company_name = trimmed
                .trim_start_matches("- **Company**:")
                .trim()
                .to_string();
        } else if trimmed.starts_with("- **Mission**:") {
            mission = trimmed
                .trim_start_matches("- **Mission**:")
                .trim()
                .to_string();
        } else if trimmed.starts_with("- **Status**:") {
            let s = trimmed
                .trim_start_matches("- **Status**:")
                .trim()
                .to_uppercase();
            status = match s.as_str() {
                "RUNNING" => ProjectStatus::Running,
                "PAUSED" => ProjectStatus::Paused,
                "STOPPED" => ProjectStatus::Stopped,
                "ERROR" => ProjectStatus::Error,
                _ => ProjectStatus::Initializing,
            };
        } else if trimmed.starts_with("- **Cycle**:") {
            cycle = trimmed
                .trim_start_matches("- **Cycle**:")
                .trim()
                .parse()
                .unwrap_or(0);
        } else if trimmed.starts_with("- **Revenue**:") {
            revenue = trimmed
                .trim_start_matches("- **Revenue**:")
                .trim()
                .to_string();
        } else if trimmed == "## Current Focus" {
            in_focus = true;
            in_next = false;
        } else if trimmed == "## Next Action" {
            in_next = true;
            in_focus = false;
        } else if trimmed.starts_with("## ") {
            in_focus = false;
            in_next = false;
        } else if in_focus && !trimmed.is_empty() {
            if !current_focus.is_empty() {
                current_focus.push(' ');
            }
            current_focus.push_str(trimmed);
        } else if in_next && !trimmed.is_empty() {
            if !next_action.is_empty() {
                next_action.push(' ');
            }
            next_action.push_str(trimmed);
        }
    }

    Ok(ConsensusState {
        company_name,
        mission,
        status,
        cycle,
        revenue,
        current_focus,
        active_projects,
        next_action,
        raw_content: content,
    })
}

pub fn update_consensus(project_dir: &Path, content: &str) -> Result<(), String> {
    let path = project_dir.join("memories/consensus.md");

    // Backup first
    let backup_path = project_dir.join("memories/consensus.md.bak");
    if path.exists() {
        fs::copy(&path, &backup_path).map_err(|e| format!("Failed to backup consensus: {}", e))?;
    }

    fs::write(&path, content).map_err(|e| format!("Failed to write consensus: {}", e))?;

    Ok(())
}

pub fn backup_consensus(project_dir: &Path) -> Result<String, String> {
    let path = project_dir.join("memories/consensus.md");
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let backup_path = project_dir.join(format!("memories/consensus_{}.md.bak", timestamp));

    fs::copy(&path, &backup_path).map_err(|e| format!("Failed to backup: {}", e))?;

    Ok(backup_path.display().to_string())
}
