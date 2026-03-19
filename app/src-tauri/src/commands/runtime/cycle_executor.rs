use super::credentials::ApiCredentials;
use super::prompt_builder::{build_system_prompt, build_user_prompt};
use crate::engine::{api_client, extract, state};
use crate::models::CycleResult;
use std::path::Path;

// ===== Skill Request Queue =====

use std::collections::HashMap;
use std::sync::Mutex;

pub(crate) static PENDING_SKILL_REQUESTS: std::sync::LazyLock<Mutex<HashMap<String, Vec<String>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

fn queue_skill_requests(project_dir: &str, skill_ids: &[String]) {
    if let Ok(mut map) = PENDING_SKILL_REQUESTS.lock() {
        let entry = map.entry(project_dir.to_string()).or_default();
        for id in skill_ids {
            if !entry.contains(id) {
                entry.push(id.clone());
            }
        }
    }
}

fn drain_pending_skills(project_dir: &str) -> Vec<String> {
    if let Ok(mut map) = PENDING_SKILL_REQUESTS.lock() {
        map.remove(project_dir).unwrap_or_default()
    } else {
        Vec::new()
    }
}

// ===== API Cycle Execution =====

#[tracing::instrument(skip(credentials), fields(agent = %agent_role, cycle = %cycle))]
pub(crate) fn run_api_cycle(
    dir: &Path,
    project_dir: &str,
    credentials: &ApiCredentials,
    agent_role: &str,
    cycle: u32,
    timeout_secs: u32,
) -> Result<(String, u32, u32), String> {
    // 1. Read agent file
    let agent_content = read_agent_file(dir, agent_role)?;

    // 2. Read current consensus
    let consensus_content = std::fs::read_to_string(dir.join("memories/consensus.md"))
        .map_err(|e| format!("Failed to read consensus: {}", e))?;

    // 3. Load agent memory and handoff note
    let agent_memory = load_agent_memory(dir, agent_role);
    let handoff_note = load_handoff(dir);

    // 4. Drain pending skill requests for injection
    let injected_skills = drain_pending_skills(project_dir);

    // 5. Build prompts
    let system_prompt = build_system_prompt(
        &agent_content,
        agent_role,
        cycle,
        &agent_memory,
        &injected_skills,
    );
    let user_prompt = build_user_prompt(&consensus_content, &handoff_note);

    // 6. Call API
    let api_config = api_client::ApiCallConfig {
        api_key: credentials.api_key.clone(),
        api_base_url: credentials.api_base_url.clone(),
        model: credentials.model.clone(),
        system_prompt,
        user_message: user_prompt,
        timeout_secs,
        anthropic_version: credentials.anthropic_version.clone(),
        extra_headers: credentials.extra_headers.clone(),
        force_stream: credentials.force_stream,
        api_format: if credentials.engine_type == "openai" {
            "openai".to_string()
        } else {
            credentials.api_format.clone()
        },
    };

    state::append_log(
        dir,
        &format!(
            "API call: engine={} model={} format={} stream={} url={}",
            credentials.engine_type,
            credentials.model,
            api_config.api_format,
            api_config.force_stream,
            credentials.api_base_url,
        ),
    );

    let response = api_client::call_api(&api_config)?;

    // 7. Extract and apply consensus update
    if let Some(updated_consensus) = extract::extract_consensus_update(&response.text) {
        let backup_path = dir.join("memories/consensus.md.bak");
        let _ = std::fs::copy(dir.join("memories/consensus.md"), &backup_path);

        std::fs::write(dir.join("memories/consensus.md"), &updated_consensus)
            .map_err(|e| format!("Failed to write consensus: {}", e))?;

        state::append_log(dir, &format!("Consensus updated by {} agent", agent_role));
    } else {
        state::append_log(
            dir,
            "No structured consensus update in response (logged only)",
        );
    }

    // 8. Extract and save agent reflection/memory and handoff note
    let reflection = extract::extract_reflection(&response.text);
    let new_handoff = extract::extract_handoff(&response.text);

    if let Some(ref refl) = reflection {
        append_agent_memory(dir, agent_role, cycle, refl);
        state::append_log(
            dir,
            &format!("Agent {} saved reflection to memory", agent_role),
        );
    }

    if let Some(ref handoff) = new_handoff {
        save_handoff(dir, agent_role, cycle, handoff);
        state::append_log(
            dir,
            &format!("Agent {} left handoff note for next agent", agent_role),
        );
    } else {
        let auto_handoff = extract::truncate_string(&response.text, 500);
        save_handoff(dir, agent_role, cycle, &auto_handoff);
    }

    // 9. Check for skill requests
    let skill_requests = extract::extract_skill_requests(&response.text);
    if !skill_requests.is_empty() {
        state::append_log(
            dir,
            &format!(
                "Agent {} requested skills: {}",
                agent_role,
                skill_requests.join(", ")
            ),
        );
        queue_skill_requests(project_dir, &skill_requests);
    }

    Ok((response.text, response.input_tokens, response.output_tokens))
}

// ===== Agent File Reading =====

fn read_agent_file(dir: &Path, role: &str) -> Result<String, String> {
    let agents_dir = dir.join(".claude/agents");
    let prefix = format!("{}-", role);

    if let Ok(entries) = std::fs::read_dir(&agents_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) && name.ends_with(".md") {
                return std::fs::read_to_string(entry.path())
                    .map_err(|e| format!("Failed to read agent file: {}", e));
            }
        }
    }

    Ok(format!(
        "# Agent: {role}\n\nYou are the {role} agent. Analyze the company state and provide \
         recommendations from your area of expertise.",
        role = role
    ))
}

// ===== Workspace-as-Memory =====

pub(crate) fn load_agent_memory(dir: &Path, role: &str) -> String {
    let memory_path = dir.join(format!("memories/agents/{}/MEMORY.md", role));
    if !memory_path.exists() {
        return String::new();
    }

    match std::fs::read_to_string(&memory_path) {
        Ok(content) => {
            let entries: Vec<&str> = content.split("\n---\n").collect();
            let start = if entries.len() > 5 {
                entries.len() - 5
            } else {
                0
            };
            entries[start..].join("\n---\n")
        }
        Err(_) => String::new(),
    }
}

fn append_agent_memory(dir: &Path, role: &str, cycle: u32, reflection: &str) {
    let memory_dir = dir.join(format!("memories/agents/{}", role));
    let _ = std::fs::create_dir_all(&memory_dir);

    let memory_path = memory_dir.join("MEMORY.md");
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();

    let entry = format!(
        "\n---\n**Cycle {} | {}**\n\n{}\n",
        cycle, timestamp, reflection
    );

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&memory_path)
    {
        use std::io::Write;
        let _ = file.write_all(entry.as_bytes());
    }
}

pub(crate) fn load_handoff(dir: &Path) -> String {
    let handoff_path = dir.join("memories/HANDOFF.md");
    std::fs::read_to_string(&handoff_path).unwrap_or_default()
}

fn save_handoff(dir: &Path, from_role: &str, cycle: u32, note: &str) {
    let handoff_path = dir.join("memories/HANDOFF.md");
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let content = format!(
        "**From: {} | Cycle {} | {}**\n\n{}",
        from_role, cycle, timestamp, note
    );
    let _ = std::fs::write(handoff_path, content);
}

// ===== Cycle History =====

pub(crate) fn load_cycle_history(dir: &Path) -> Vec<CycleResult> {
    let path = dir.join(".cycle_history.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

pub(crate) fn save_cycle_history(dir: &Path, history: &[CycleResult]) {
    let path = dir.join(".cycle_history.json");
    if let Ok(json) = serde_json::to_string_pretty(history) {
        let _ = std::fs::write(path, json);
    }
}
