use crate::models::*;
use std::fs;
use tauri::command;

// ===== GitHub API Helpers =====

fn github_contents_url(owner: &str, repo: &str, path: &str, branch: &str) -> String {
    format!(
        "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
        owner, repo, path, branch
    )
}

fn github_api_get(url: &str) -> Result<serde_json::Value, String> {
    let resp = ureq::get(url)
        .set("Accept", "application/vnd.github.v3+json")
        .set("User-Agent", "omnihive")
        .call()
        .map_err(|e| format!("GitHub API error: {}", e))?;
    resp.into_json::<serde_json::Value>()
        .map_err(|e| format!("JSON parse error: {}", e))
}

fn github_raw_get(url: &str) -> Result<String, String> {
    let resp = ureq::get(url)
        .set("User-Agent", "omnihive")
        .call()
        .map_err(|e| format!("Download error: {}", e))?;
    resp.into_string().map_err(|e| format!("Read error: {}", e))
}

// ===== Repo CRUD Commands =====

#[command]
pub fn list_skill_repos() -> Result<Vec<SkillRepo>, String> {
    let settings = crate::commands::settings::load_settings()?;
    Ok(settings.skill_repos)
}

#[command]
pub fn add_skill_repo(repo: SkillRepo) -> Result<AppSettings, String> {
    let mut settings = crate::commands::settings::load_settings()?;

    if settings.skill_repos.iter().any(|r| r.id == repo.id) {
        return Err(format!("Repository '{}' already exists", repo.id));
    }

    settings.skill_repos.push(repo);
    crate::commands::settings::save_settings(settings.clone())?;
    Ok(settings)
}

#[command]
pub fn remove_skill_repo(repo_id: String) -> Result<AppSettings, String> {
    let mut settings = crate::commands::settings::load_settings()?;
    settings.skill_repos.retain(|r| r.id != repo_id);
    crate::commands::settings::save_settings(settings.clone())?;
    Ok(settings)
}

// ===== Repo Browsing =====

#[command]
pub fn browse_repo(repo_id: String, subpath: String) -> Result<Vec<RepoItem>, String> {
    let settings = crate::commands::settings::load_settings()?;
    let repo = settings
        .skill_repos
        .iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repository '{}' not found", repo_id))?;

    let full_path = if subpath.is_empty() {
        repo.path.clone()
    } else if repo.path.is_empty() {
        subpath
    } else {
        format!("{}/{}", repo.path, subpath)
    };

    let url = github_contents_url(&repo.owner, &repo.repo, &full_path, &repo.branch);
    let json = github_api_get(&url)?;

    let items = json
        .as_array()
        .ok_or_else(|| "Expected array from GitHub API".to_string())?;

    let mut results = Vec::new();
    for item in items {
        let name = item["name"].as_str().unwrap_or("").to_string();
        let path = item["path"].as_str().unwrap_or("").to_string();
        let item_type = item["type"].as_str().unwrap_or("file").to_string();
        let download_url = item["download_url"].as_str().map(|s| s.to_string());

        // Skip hidden files/dirs
        if name.starts_with('.') {
            continue;
        }

        results.push(RepoItem {
            name,
            path,
            item_type,
            download_url,
            description: String::new(),
        });
    }

    // Sort: directories first, then alphabetical
    results.sort_by(|a, b| {
        let dir_cmp = (a.item_type != "dir").cmp(&(b.item_type != "dir"));
        dir_cmp.then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(results)
}

/// Browse a repo and try to load descriptions from SKILL.md in each subdirectory.
#[command]
pub fn browse_repo_skills(repo_id: String) -> Result<Vec<RepoItem>, String> {
    let settings = crate::commands::settings::load_settings()?;
    let repo = settings
        .skill_repos
        .iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repository '{}' not found", repo_id))?;

    let url = github_contents_url(&repo.owner, &repo.repo, &repo.path, &repo.branch);
    let json = github_api_get(&url)?;

    let items = json
        .as_array()
        .ok_or_else(|| "Expected array from GitHub API".to_string())?;

    let mut results = Vec::new();
    for item in items {
        let name = item["name"].as_str().unwrap_or("").to_string();
        let path = item["path"].as_str().unwrap_or("").to_string();
        let item_type = item["type"].as_str().unwrap_or("file").to_string();

        // Only show directories (skill folders)
        if item_type != "dir" || name.starts_with('.') {
            continue;
        }

        // Try to fetch SKILL.md description
        let skill_md_url = github_contents_url(
            &repo.owner,
            &repo.repo,
            &format!("{}/SKILL.md", path),
            &repo.branch,
        );
        let description = match github_api_get(&skill_md_url) {
            Ok(skill_json) => {
                if let Some(download) = skill_json["download_url"].as_str() {
                    match github_raw_get(download) {
                        Ok(content) => parse_first_paragraph(&content),
                        Err(_) => String::new(),
                    }
                } else {
                    String::new()
                }
            }
            Err(_) => String::new(),
        };

        results.push(RepoItem {
            name,
            path,
            item_type: "dir".to_string(),
            download_url: None,
            description,
        });
    }

    results.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(results)
}

// ===== Install Skill from Repo =====

#[command]
pub fn install_repo_skill(repo_id: String, skill_path: String) -> Result<SkillInfo, String> {
    let settings = crate::commands::settings::load_settings()?;
    let repo = settings
        .skill_repos
        .iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repository '{}' not found", repo_id))?;

    // Get the directory listing for this skill
    let url = github_contents_url(&repo.owner, &repo.repo, &skill_path, &repo.branch);
    let json = github_api_get(&url)?;

    let items = json
        .as_array()
        .ok_or_else(|| "Expected array from GitHub API".to_string())?;

    // Determine skill name from path
    let skill_name = skill_path
        .split('/')
        .next_back()
        .unwrap_or("unknown-skill")
        .to_string();

    // Create the local skill directory
    let install_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("omnihive")
        .join("custom-skills")
        .join(&skill_name);

    fs::create_dir_all(&install_dir)
        .map_err(|e| format!("Failed to create skill directory: {}", e))?;

    let mut description = String::new();
    let mut content_preview = String::new();

    // Download all files in the skill directory
    for item in items {
        let file_name = item["name"].as_str().unwrap_or("");
        let download_url = match item["download_url"].as_str() {
            Some(u) => u,
            None => continue,
        };

        // Only download files, not subdirectories
        if item["type"].as_str() != Some("file") {
            continue;
        }

        let content = github_raw_get(download_url)?;

        // Parse SKILL.md for metadata
        if file_name == "SKILL.md" {
            let (parsed_desc, _) = parse_skill_md_content(&content);
            description = parsed_desc;
            content_preview = content.chars().take(200).collect();
        }

        let file_path = install_dir.join(file_name);
        fs::write(&file_path, &content)
            .map_err(|e| format!("Failed to write {}: {}", file_name, e))?;
    }

    Ok(SkillInfo {
        id: format!("custom:{}", skill_name),
        name: skill_name,
        category: format!("repo:{}", repo.name),
        description,
        source: "custom".to_string(),
        content_preview,
        enabled: true,
        file_path: Some(install_dir.display().to_string()),
        tags: vec![format!("from:{}", repo.name)],
    })
}

// ===== Helpers =====

fn parse_first_paragraph(content: &str) -> String {
    let mut found_header = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if !found_header {
            if trimmed.starts_with("# ") {
                found_header = true;
            }
        } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
            return trimmed.to_string();
        }
    }
    String::new()
}

fn parse_skill_md_content(content: &str) -> (String, String) {
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
            description = trimmed.to_string();
        }
        if !name.is_empty() && !description.is_empty() {
            break;
        }
    }

    (description, name)
}
