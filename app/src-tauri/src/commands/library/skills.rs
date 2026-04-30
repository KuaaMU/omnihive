use super::registry::get_library_dir;
use crate::models::SkillInfo;

#[derive(serde::Deserialize)]
struct SkillYaml {
    id: String,
    name: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    capabilities: Vec<String>,
}

pub(crate) fn load_skills_from_files() -> Option<Vec<SkillInfo>> {
    let lib_dir = get_library_dir()?;
    let mut skills = Vec::new();

    // 1. Load from library/skills/*.yaml
    let skills_dir = lib_dir.join("skills");
    if skills_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if name.starts_with('_') || !name.ends_with(".yaml") {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(yaml) = serde_yaml::from_str::<SkillYaml>(&content) {
                        skills.push(SkillInfo {
                            id: yaml.id,
                            name: yaml.name,
                            category: yaml.category,
                            description: yaml.description,
                            source: "auto-company".to_string(),
                            content_preview: yaml.capabilities.first().cloned().unwrap_or_default(),
                            enabled: true,
                            file_path: Some(path.display().to_string()),
                            tags: vec![],
                        });
                    }
                }
            }
        }
    }

    // 2. Load from library/real-skills/*/SKILL.md
    load_skills_from_dir(
        &lib_dir,
        "real-skills",
        "General",
        "real-skills",
        &mut skills,
    );

    // 3. Load from library/ecc-skills/*/SKILL.md
    load_skills_from_dir(&lib_dir, "ecc-skills", "Engineering", "ecc", &mut skills);

    if skills.is_empty() {
        None
    } else {
        Some(skills)
    }
}

fn load_skills_from_dir(
    lib_dir: &std::path::Path,
    dir_name: &str,
    default_category: &str,
    source: &str,
    skills: &mut Vec<SkillInfo>,
) {
    let dir = lib_dir.join(dir_name);
    if !dir.exists() {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let dir_path = entry.path();
            if !dir_path.is_dir() {
                continue;
            }
            let skill_md = dir_path.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }
            let id = dir_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            if skills.iter().any(|s| s.id == id) {
                if let Some(existing) = skills.iter_mut().find(|s| s.id == id) {
                    existing.file_path = Some(skill_md.display().to_string());
                }
                continue;
            }

            if let Ok(content) = std::fs::read_to_string(&skill_md) {
                let (name, desc) = parse_skill_md_frontmatter(&content);
                let preview = content
                    .lines()
                    .filter(|l| {
                        !l.starts_with('#') && !l.starts_with("---") && !l.trim().is_empty()
                    })
                    .take(1)
                    .collect::<Vec<_>>()
                    .join("");
                skills.push(SkillInfo {
                    id,
                    name,
                    category: default_category.to_string(),
                    description: desc,
                    source: source.to_string(),
                    content_preview: truncate(&preview, 150),
                    enabled: true,
                    file_path: Some(skill_md.display().to_string()),
                    tags: vec![],
                });
            }
        }
    }
}

fn parse_skill_md_frontmatter(content: &str) -> (String, String) {
    let mut name = String::new();
    let mut desc = String::new();

    if content.starts_with("---") {
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() >= 3 {
            for line in parts[1].lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("name:") {
                    name = rest.trim().to_string();
                } else if let Some(rest) = trimmed.strip_prefix("description:") {
                    desc = rest.trim().to_string();
                }
            }
        }
    }

    if name.is_empty() {
        for line in content.lines() {
            if let Some(heading) = line.strip_prefix("# ") {
                name = heading.trim().to_string();
                break;
            }
        }
    }

    if name.is_empty() {
        name = "Unnamed Skill".to_string();
    }

    (name, desc)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

fn validate_skill_id(skill_id: &str) -> Result<(), String> {
    if skill_id.is_empty() {
        return Err("Skill ID cannot be empty".to_string());
    }
    if !skill_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("Skill ID contains invalid characters".to_string());
    }
    Ok(())
}

pub(crate) fn get_skill_content_impl(skill_id: &str) -> Result<String, String> {
    validate_skill_id(skill_id)?;
    let lib_dir = get_library_dir().ok_or_else(|| "Library directory not found".to_string())?;

    let real_path = lib_dir.join("real-skills").join(skill_id).join("SKILL.md");
    if real_path.exists() {
        return std::fs::read_to_string(&real_path)
            .map_err(|e| format!("Failed to read skill: {}", e));
    }

    let ecc_path = lib_dir.join("ecc-skills").join(skill_id).join("SKILL.md");
    if ecc_path.exists() {
        return std::fs::read_to_string(&ecc_path)
            .map_err(|e| format!("Failed to read skill: {}", e));
    }

    let yaml_path = lib_dir.join("skills").join(format!("{}.yaml", skill_id));
    if yaml_path.exists() {
        return std::fs::read_to_string(&yaml_path)
            .map_err(|e| format!("Failed to read skill: {}", e));
    }

    Err(format!("Skill '{}' not found in library", skill_id))
}

pub(crate) fn fallback_skills() -> Vec<SkillInfo> {
    let (enabled, file_path, tags) = super::default_lib_fields();
    let mut skills = Vec::new();

    let auto_company = vec![
        (
            "deep-research",
            "Research",
            "Comprehensive research methodology",
        ),
        (
            "product-strategist",
            "Product",
            "Product strategy framework",
        ),
        ("market-sizing", "Business", "TAM/SAM/SOM market sizing"),
        (
            "startup-financial-modeling",
            "Finance",
            "Financial modeling for startups",
        ),
        (
            "micro-saas-launcher",
            "Operations",
            "Micro-SaaS launch playbook",
        ),
        ("premortem", "Strategy", "Pre-mortem analysis"),
        (
            "code-review-security",
            "Engineering",
            "Security-focused code review",
        ),
        ("devops", "Engineering", "DevOps pipeline setup"),
        ("senior-qa", "Engineering", "Senior QA testing strategy"),
        ("security-audit", "Security", "Security audit framework"),
        (
            "competitive-intelligence",
            "Business",
            "Competitive intelligence",
        ),
        (
            "financial-unit-economics",
            "Finance",
            "Unit economics analysis",
        ),
        (
            "seo-content-strategist",
            "Marketing",
            "SEO and content strategy",
        ),
        ("pricing-strategy", "Business", "Pricing strategy framework"),
        ("web-scraping", "Engineering", "Web scraping tools"),
    ];

    for (id, category, description) in auto_company {
        skills.push(SkillInfo {
            id: id.into(),
            name: id.replace('-', " "),
            category: category.into(),
            description: description.into(),
            source: "auto-company".into(),
            content_preview: String::new(),
            enabled,
            file_path: file_path.clone(),
            tags: tags.clone(),
        });
    }

    let ecc = vec![
        (
            "tdd-workflow",
            "Engineering",
            "Test-driven development workflow",
        ),
        (
            "security-review",
            "Security",
            "Security vulnerability review",
        ),
        ("security-scan", "Security", "Automated security scanning"),
        ("python-patterns", "Engineering", "Python design patterns"),
        ("golang-patterns", "Engineering", "Go design patterns"),
        (
            "postgres-patterns",
            "Engineering",
            "PostgreSQL query patterns",
        ),
        (
            "docker-patterns",
            "Engineering",
            "Docker containerization patterns",
        ),
        (
            "api-design",
            "Engineering",
            "REST API design best practices",
        ),
        (
            "frontend-patterns",
            "Engineering",
            "Frontend architecture patterns",
        ),
        (
            "backend-patterns",
            "Engineering",
            "Backend architecture patterns",
        ),
        (
            "e2e-testing",
            "Engineering",
            "End-to-end testing strategies",
        ),
        ("coding-standards", "Engineering", "Code quality standards"),
        (
            "verification-loop",
            "Engineering",
            "Verification loop for code changes",
        ),
    ];

    for (id, category, description) in ecc {
        skills.push(SkillInfo {
            id: id.into(),
            name: id.replace('-', " "),
            category: category.into(),
            description: description.into(),
            source: "ecc".into(),
            content_preview: String::new(),
            enabled,
            file_path: file_path.clone(),
            tags: tags.clone(),
        });
    }

    skills
}
