use std::fs;
use std::path::Path;
use crate::models::*;

pub fn generate_all(
    config: &FactoryConfig,
    output_dir: &Path,
    _templates_dir: &Path,
) -> Result<GenerateResult, String> {
    let mut files_created = Vec::new();

    // Create directory structure
    let dirs = [
        output_dir.join(".claude"),
        output_dir.join(".claude/agents"),
        output_dir.join("memories"),
        output_dir.join("docs"),
        output_dir.join("projects"),
        output_dir.join("logs"),
        output_dir.join("scripts"),
    ];
    for dir in &dirs {
        fs::create_dir_all(dir).map_err(|e| format!("Failed to create dir: {}", e))?;
    }

    // Create doc dirs for each agent
    for agent in &config.org.agents {
        let doc_dir = output_dir.join(format!("docs/{}", agent.role));
        fs::create_dir_all(&doc_dir).map_err(|e| format!("Failed to create doc dir: {}", e))?;
    }

    // 1. Generate company.yaml
    let yaml_content = serde_yaml::to_string(config)
        .map_err(|e| format!("YAML serialize error: {}", e))?;
    let config_path = output_dir.join("company.yaml");
    fs::write(&config_path, &yaml_content).map_err(|e| format!("Write error: {}", e))?;
    files_created.push(config_path.display().to_string());

    // 2. Generate CLAUDE.md
    let claude_md = generate_claude_md(config);
    let claude_path = output_dir.join("CLAUDE.md");
    fs::write(&claude_path, &claude_md).map_err(|e| format!("Write error: {}", e))?;
    files_created.push(claude_path.display().to_string());

    // 3. Generate agent files
    for agent in &config.org.agents {
        let agent_md = generate_agent_md(agent, config);
        let path = output_dir.join(format!(".claude/agents/{}-{}.md", agent.role, agent.persona.id));
        fs::write(&path, &agent_md).map_err(|e| format!("Write error: {}", e))?;
        files_created.push(path.display().to_string());
    }

    // 4. Generate consensus.md
    let consensus = generate_consensus_md(config);
    let consensus_path = output_dir.join("memories/consensus.md");
    fs::write(&consensus_path, &consensus).map_err(|e| format!("Write error: {}", e))?;
    files_created.push(consensus_path.display().to_string());

    // 5. Generate .claude/settings.json
    let settings = generate_settings_json(config);
    let settings_path = output_dir.join(".claude/settings.json");
    fs::write(
        &settings_path,
        serde_json::to_string_pretty(&settings).unwrap(),
    ).map_err(|e| format!("Write error: {}", e))?;
    files_created.push(settings_path.display().to_string());

    // 6. Generate workflow docs
    for workflow in &config.workflows {
        let wf_md = generate_workflow_md(workflow);
        let path = output_dir.join(format!("docs/workflow-{}.md", workflow.id));
        fs::write(&path, &wf_md).map_err(|e| format!("Write error: {}", e))?;
        files_created.push(path.display().to_string());
    }

    // 7. Generate auto-loop script
    let loop_script = generate_loop_script(config);
    let script_path = output_dir.join("scripts/auto-loop.sh");
    fs::write(&script_path, &loop_script).map_err(|e| format!("Write error: {}", e))?;
    files_created.push(script_path.display().to_string());

    // 8. Initialize state files
    let state_content = "current_cycle=0\ntotal_cycles=0\nconsecutive_errors=0\nstatus=stopped\n";
    let state_path = output_dir.join(".loop.state");
    fs::write(&state_path, state_content).map_err(|e| format!("Write error: {}", e))?;
    files_created.push(state_path.display().to_string());

    let history_path = output_dir.join(".cycle_history.json");
    fs::write(&history_path, "[]").map_err(|e| format!("Write error: {}", e))?;
    files_created.push(history_path.display().to_string());

    // 9. Create empty log file
    let log_path = output_dir.join("logs/auto-loop.log");
    fs::write(&log_path, "").map_err(|e| format!("Write error: {}", e))?;
    files_created.push(log_path.display().to_string());

    let unique_skills: std::collections::HashSet<_> = config.org.agents.iter()
        .flat_map(|a| &a.skills)
        .collect();

    Ok(GenerateResult {
        output_dir: output_dir.display().to_string(),
        files_created,
        agent_count: config.org.agents.len(),
        skill_count: unique_skills.len(),
        workflow_count: config.workflows.len(),
    })
}

fn generate_claude_md(config: &FactoryConfig) -> String {
    let mut md = String::new();

    md.push_str(&format!("# {}\n\n", config.company.name));
    md.push_str(&format!("## Mission\n\n{}\n\n", config.company.mission));
    md.push_str(&format!("## Description\n\n{}\n\n", config.company.description));

    // Team overview
    md.push_str("## Team\n\n");
    md.push_str("| Role | Persona | Layer | Model |\n");
    md.push_str("|------|---------|-------|-------|\n");
    for agent in &config.org.agents {
        md.push_str(&format!(
            "| {} | {} | {:?} | {:?} |\n",
            agent.role, agent.persona.id, agent.layer, agent.model
        ));
    }
    md.push('\n');

    // Workflows
    if !config.workflows.is_empty() {
        md.push_str("## Workflows\n\n");
        for wf in &config.workflows {
            md.push_str(&format!(
                "### {}\n{}\n\nChain: {}\n\n",
                wf.name, wf.description, wf.chain.join(" -> ")
            ));
        }
    }

    // Operating rules
    md.push_str("## Operating Rules\n\n");
    md.push_str("1. Read `memories/consensus.md` at the start of every cycle\n");
    md.push_str("2. Perform your role's designated task\n");
    md.push_str("3. Update `memories/consensus.md` with your findings/decisions\n");
    md.push_str("4. Stay within the workspace boundary\n");
    md.push_str("5. Never execute forbidden commands\n\n");

    // Guardrails
    md.push_str("## Guardrails\n\n");
    md.push_str("### Forbidden Commands\n\n");
    for cmd in &config.guardrails.forbidden {
        md.push_str(&format!("- `{}`\n", cmd));
    }
    md.push_str(&format!("\n### Workspace: `{}`\n", config.guardrails.workspace));
    md.push_str(&format!(
        "### Critic Review Required: {}\n\n",
        if config.guardrails.require_critic_review { "Yes" } else { "No" }
    ));

    // Budget
    md.push_str("## Budget\n\n");
    md.push_str(&format!("- Max Daily: ${:.2}\n", config.runtime.budget.max_daily_usd));
    md.push_str(&format!("- Alert At: ${:.2}\n", config.runtime.budget.alert_at_usd));

    md
}

fn generate_agent_md(agent: &AgentConfig, config: &FactoryConfig) -> String {
    let mut md = String::new();

    md.push_str(&format!("# Agent: {} ({})\n\n", agent.role, agent.persona.id));
    md.push_str(&format!("**Company**: {}\n", config.company.name));
    md.push_str(&format!("**Mission**: {}\n\n", config.company.mission));

    md.push_str(&format!("## Role: {}\n\n", agent.role));
    md.push_str(&format!("**Layer**: {:?}\n", agent.layer));
    md.push_str(&format!("**Model**: {:?}\n\n", agent.model));

    // Persona instructions
    md.push_str("## Persona\n\n");
    md.push_str(&format!("You are channeling the expertise of **{}**.\n", agent.persona.id));
    md.push_str("Apply their mental models, decision-making frameworks, and expertise to every task.\n\n");

    if !agent.persona.custom_instructions.is_empty() {
        md.push_str(&format!("### Custom Instructions\n\n{}\n\n", agent.persona.custom_instructions));
    }

    // Skills
    if !agent.skills.is_empty() {
        md.push_str("## Skills\n\n");
        for skill in &agent.skills {
            md.push_str(&format!("- {}\n", skill));
        }
        md.push('\n');
    }

    // Operational protocol
    md.push_str("## Operational Protocol\n\n");
    md.push_str("1. **Read Consensus**: Start by reading `memories/consensus.md`\n");
    md.push_str("2. **Assess**: Determine what needs to be done from your role's perspective\n");
    md.push_str("3. **Act**: Execute your designated task using your skills\n");
    md.push_str("4. **Update**: Write your findings/decisions back to consensus\n");
    md.push_str("5. **Document**: Log important decisions in the decision log table\n\n");

    // Decides
    if !agent.decides.is_empty() {
        md.push_str("## Decision Authority\n\n");
        for d in &agent.decides {
            md.push_str(&format!("- {}\n", d));
        }
        md.push('\n');
    }

    // Guardrails
    md.push_str("## Safety\n\n");
    md.push_str("You MUST NOT execute any of these commands:\n\n");
    for cmd in &config.guardrails.forbidden {
        md.push_str(&format!("- `{}`\n", cmd));
    }
    md.push_str(&format!("\nStay within workspace: `{}`\n", config.guardrails.workspace));

    md
}

fn generate_consensus_md(config: &FactoryConfig) -> String {
    format!(
        r#"# Auto Company Consensus

## Company State

- **Company**: {}
- **Mission**: {}
- **Status**: INITIALIZING
- **Cycle**: 0
- **Revenue**: $0

## Current Focus

Starting up. First cycle should brainstorm product ideas aligned with our mission.

Seed direction: {}

## Active Projects

None yet. First cycle will identify opportunities.

## Next Action

**Brainstorm Phase**: Each team member proposes their best product idea based on our mission.

## Decision Log

| Cycle | Decision | Made By | Outcome |
|-------|----------|---------|---------|
| 0 | Company initialized | System | Pending first cycle |
"#,
        config.company.name, config.company.mission, config.company.seed_prompt
    )
}

fn generate_settings_json(config: &FactoryConfig) -> serde_json::Value {
    serde_json::json!({
        "permissions": {
            "allow": [
                "Bash(npm install:*)",
                "Bash(npm run:*)",
                "Bash(git:*)",
                "Bash(mkdir:*)",
                "Bash(cp:*)",
                "Bash(mv:*)",
                "Bash(curl:*)",
                "WebFetch",
                "WebSearch"
            ],
            "deny": config.guardrails.forbidden
        }
    })
}

fn generate_workflow_md(workflow: &WorkflowConfig) -> String {
    let mut md = String::new();

    md.push_str(&format!("# Workflow: {}\n\n", workflow.name));
    md.push_str(&format!("**ID**: {}\n", workflow.id));
    md.push_str(&format!("**Description**: {}\n\n", workflow.description));
    md.push_str("## Chain\n\n");
    for (i, role) in workflow.chain.iter().enumerate() {
        md.push_str(&format!("{}. **{}**\n", i + 1, role));
    }
    md.push_str(&format!(
        "\n**Convergence Cycles**: {}\n",
        workflow.convergence_cycles
    ));

    md
}

fn generate_loop_script(config: &FactoryConfig) -> String {
    let agent_roles: Vec<&str> = config.org.agents.iter().map(|a| a.role.as_str()).collect();

    format!(
        r#"#!/usr/bin/env bash
# Auto-loop script for {}
# Generated by Omnihive

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
STATE_FILE="$PROJECT_DIR/.loop.state"
HISTORY_FILE="$PROJECT_DIR/.cycle_history.json"
LOG_FILE="$PROJECT_DIR/logs/auto-loop.log"
CONSENSUS="$PROJECT_DIR/memories/consensus.md"

ENGINE="${{ENGINE:-claude}}"
MODEL="${{MODEL:-sonnet}}"
MAX_ERRORS={}
LOOP_INTERVAL={}
CYCLE_TIMEOUT={}

CYCLE=0
ERRORS=0
AGENTS=({})

log() {{
    local timestamp
    timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] $1" >> "$LOG_FILE"
    echo "[$timestamp] $1"
}}

update_state() {{
    cat > "$STATE_FILE" << EOF
current_cycle=$CYCLE
total_cycles=$CYCLE
consecutive_errors=$ERRORS
status=$1
last_cycle_at=$(date -Iseconds)
EOF
}}

log "Starting auto-loop for {}"
log "Engine: $ENGINE | Model: $MODEL | Agents: ${{#AGENTS[@]}}"
update_state "running"

while true; do
    CYCLE=$((CYCLE + 1))
    AGENT_IDX=$(( (CYCLE - 1) % ${{#AGENTS[@]}} ))
    CURRENT_AGENT="${{AGENTS[$AGENT_IDX]}}"

    log "=== Cycle $CYCLE: Agent $CURRENT_AGENT ==="

    AGENT_FILE="$PROJECT_DIR/.claude/agents/$CURRENT_AGENT-*.md"
    AGENT_FILES=( $AGENT_FILE )

    if [ ! -f "${{AGENT_FILES[0]:-}}" ]; then
        log "WARNING: No agent file for $CURRENT_AGENT, skipping"
        continue
    fi

    STARTED_AT=$(date -Iseconds)

    PROMPT="You are the $CURRENT_AGENT agent. Read memories/consensus.md, perform your role, and update consensus with your findings."

    if timeout "$CYCLE_TIMEOUT" "$ENGINE" --print --model "$MODEL" "$PROMPT" >> "$LOG_FILE" 2>&1; then
        ERRORS=0
        log "Cycle $CYCLE completed successfully"
    else
        ERRORS=$((ERRORS + 1))
        log "ERROR: Cycle $CYCLE failed (consecutive errors: $ERRORS)"

        if [ "$ERRORS" -ge "$MAX_ERRORS" ]; then
            log "FATAL: Max consecutive errors reached ($MAX_ERRORS). Stopping."
            update_state "error"
            exit 1
        fi
    fi

    COMPLETED_AT=$(date -Iseconds)
    update_state "running"

    log "Sleeping $LOOP_INTERVAL seconds..."
    sleep "$LOOP_INTERVAL"
done
"#,
        config.company.name,
        config.runtime.max_consecutive_errors,
        config.runtime.loop_interval,
        config.runtime.cycle_timeout,
        agent_roles.join(" "),
        config.company.name,
    )
}
