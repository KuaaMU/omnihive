use std::path::Path;
use crate::engine::extract::truncate_string;

// ===== System Prompt Construction =====

pub(crate) fn build_system_prompt(
    agent_content: &str,
    role: &str,
    cycle: u32,
    agent_memory: &str,
    injected_skills: &[String],
) -> String {
    let skill_section = load_role_skills(role);

    let injected_section = if injected_skills.is_empty() {
        String::new()
    } else {
        let lib_dir = crate::commands::library::get_library_dir_pub();
        let mut sections = Vec::new();
        for skill_id in injected_skills {
            if let Some(content) = load_skill_full_content(skill_id, lib_dir.as_deref()) {
                sections.push(format!("### {} (requested)\n{}", skill_id, content));
            }
        }
        if sections.is_empty() {
            String::new()
        } else {
            format!(
                "\n\n## Injected Skills (you requested these)\n\n{}",
                sections.join("\n\n")
            )
        }
    };

    let memory_section = if agent_memory.is_empty() {
        String::new()
    } else {
        format!(
            "\n\n## Your Memory (from previous cycles)\n\n{}\n",
            agent_memory
        )
    };

    format!(
        r#"{agent_content}
{skill_section}{injected_section}{memory_section}
---

You are performing cycle {cycle} of the autonomous company loop.

YOUR TASK:
1. Read the current consensus document and the handoff note from the previous agent
2. From your perspective as the {role}, analyze the current state
3. Decide on actions aligned with the company mission
4. Output the COMPLETE updated consensus.md
5. Leave a REFLECTION about what you learned and a HANDOFF note for the next agent

If you need a specific skill not already provided, you can request it:
<<<SKILL_REQUEST>>>skill-name<<<SKILL_REQUEST_END>>>

OUTPUT FORMAT:
First, briefly state your analysis and decision (2-3 sentences).

Then output the FULL updated consensus.md between these markers:
<<<CONSENSUS_START>>>
[Full updated consensus.md content]
<<<CONSENSUS_END>>>

Then provide your reflection (what you learned, what went well/poorly):
<<<REFLECTION_START>>>
[Brief reflection on this cycle - what you decided and why, what you learned]
<<<REFLECTION_END>>>

Then leave a handoff note for the next agent:
<<<HANDOFF_START>>>
[Brief note about current priorities, blockers, and what the next agent should focus on]
<<<HANDOFF_END>>>

RULES:
- Output the COMPLETE consensus.md between the markers (not partial)
- Set the Cycle number to {cycle}
- Add your decision to the Decision Log table
- Update Current Focus and Next Action as needed
- Preserve all existing sections
- Be concise and actionable
- Your reflection will be saved to your personal memory for future cycles
- Your handoff note will be shown to the next agent in the chain"#,
        agent_content = agent_content,
        skill_section = skill_section,
        injected_section = injected_section,
        memory_section = memory_section,
        cycle = cycle,
        role = role,
    )
}

// ===== User Prompt Construction =====

pub(crate) fn build_user_prompt(consensus_content: &str, handoff_note: &str) -> String {
    if handoff_note.is_empty() {
        format!("Current consensus.md:\n\n{}", consensus_content)
    } else {
        format!(
            "## Handoff from Previous Agent\n\n{}\n\n---\n\nCurrent consensus.md:\n\n{}",
            handoff_note, consensus_content
        )
    }
}

// ===== Skill Mapping =====

fn role_to_skills(role: &str) -> Vec<&'static str> {
    match role {
        "ceo" => vec![
            "deep-research", "product-strategist", "market-sizing",
            "startup-financial-modeling", "premortem",
        ],
        "fullstack" => vec![
            "code-review-security", "tdd-workflow", "frontend-patterns",
            "backend-patterns", "api-design",
        ],
        "devops" => vec![
            "devops", "docker-patterns", "security-audit", "deployment-patterns",
        ],
        "critic" => vec![
            "premortem", "financial-unit-economics", "security-review",
        ],
        "product" => vec![
            "product-strategist", "deep-research", "market-sizing",
        ],
        "ui" => vec!["frontend-patterns", "product-strategist"],
        "qa" => vec![
            "senior-qa", "tdd-workflow", "e2e-testing", "verification-loop",
        ],
        "marketing" => vec![
            "seo-content-strategist", "competitive-intelligence", "content-strategy",
        ],
        "operations" => vec!["micro-saas-launcher", "startup-financial-modeling"],
        "sales" => vec!["competitive-intelligence", "pricing-strategy"],
        "cfo" => vec![
            "financial-unit-economics", "pricing-strategy", "startup-financial-modeling",
        ],
        "research" => vec![
            "deep-research", "competitive-intelligence", "market-sizing",
        ],
        _ => vec![],
    }
}

// ===== Skill Loading =====

fn load_role_skills(role: &str) -> String {
    let skill_ids = role_to_skills(role);
    if skill_ids.is_empty() {
        return String::new();
    }

    let lib_dir = crate::commands::library::get_library_dir_pub();
    let mut skill_sections = Vec::new();

    for skill_id in &skill_ids {
        if let Some(summary) = load_skill_summary(skill_id, lib_dir.as_deref()) {
            skill_sections.push(format!("### {}\n{}", skill_id, summary));
        }
    }

    if skill_sections.is_empty() {
        return String::new();
    }

    format!(
        "\n\n## Available Skills\n\n{}",
        skill_sections.join("\n\n")
    )
}

fn load_skill_summary(skill_id: &str, lib_dir: Option<&Path>) -> Option<String> {
    let lib = lib_dir?;

    // Try library/skills/{id}.yaml first
    let yaml_path = lib.join("skills").join(format!("{}.yaml", skill_id));
    if yaml_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&yaml_path) {
            let mut desc = String::new();
            let mut caps = Vec::new();
            let mut in_capabilities = false;

            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("description:") {
                    desc = rest.trim().trim_matches('"').to_string();
                } else if line.trim() == "capabilities:" {
                    in_capabilities = true;
                } else if in_capabilities {
                    if let Some(cap) = line.trim().strip_prefix("- ") {
                        if caps.len() < 3 {
                            caps.push(cap.trim_matches('"').to_string());
                        }
                    } else if !line.starts_with(' ') {
                        in_capabilities = false;
                    }
                }
            }

            if !desc.is_empty() {
                let cap_text = if caps.is_empty() {
                    String::new()
                } else {
                    format!("\nCapabilities: {}", caps.join("; "))
                };
                return Some(format!("{}{}", desc, cap_text));
            }
        }
    }

    // Try real-skills/{id}/SKILL.md
    let real_path = lib.join("real-skills").join(skill_id).join("SKILL.md");
    if real_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&real_path) {
            return Some(extract_skill_md_summary(&content));
        }
    }

    // Try ecc-skills/{id}/SKILL.md
    let ecc_path = lib.join("ecc-skills").join(skill_id).join("SKILL.md");
    if ecc_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&ecc_path) {
            return Some(extract_skill_md_summary(&content));
        }
    }

    None
}

fn extract_skill_md_summary(content: &str) -> String {
    let mut description = String::new();

    if content.starts_with("---") {
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() >= 3 {
            for line in parts[1].lines() {
                if let Some(rest) = line.trim().strip_prefix("description:") {
                    description = rest.trim().to_string();
                    break;
                }
            }
        }
    }

    if description.is_empty() {
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("---") {
                description = trimmed.to_string();
                break;
            }
        }
    }

    if description.len() > 300 {
        format!("{}...", &description[..300])
    } else {
        description
    }
}

pub(crate) fn load_skill_full_content(
    skill_id: &str,
    lib_dir: Option<&Path>,
) -> Option<String> {
    let lib = lib_dir?;

    let real_path = lib.join("real-skills").join(skill_id).join("SKILL.md");
    if real_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&real_path) {
            return Some(truncate_string(&content, 2000));
        }
    }

    let ecc_path = lib.join("ecc-skills").join(skill_id).join("SKILL.md");
    if ecc_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&ecc_path) {
            return Some(truncate_string(&content, 2000));
        }
    }

    load_skill_summary(skill_id, lib_dir)
}
