use crate::models::PersonaInfo;
use super::registry::get_library_dir;

#[derive(serde::Deserialize)]
struct PersonaYaml {
    id: String,
    name: String,
    #[serde(default)]
    role: String,
    #[serde(default, rename = "layer")]
    _layer: String,
    #[serde(default)]
    mental_models: Vec<String>,
    #[serde(default)]
    core_capabilities: Vec<String>,
    #[serde(default)]
    communication_style: String,
    #[serde(default)]
    recommended_skills: Vec<String>,
}

pub(crate) fn load_personas_from_files() -> Option<Vec<PersonaInfo>> {
    let lib_dir = get_library_dir()?;
    let personas_dir = lib_dir.join("personas");
    if !personas_dir.exists() {
        return None;
    }

    let mut personas = Vec::new();
    let entries = std::fs::read_dir(&personas_dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name()?.to_string_lossy().to_string();

        if name.starts_with('_') || !name.ends_with(".yaml") {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(yaml) = serde_yaml::from_str::<PersonaYaml>(&content) {
                let expertise = if !yaml.communication_style.is_empty() {
                    yaml.communication_style
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string()
                } else {
                    yaml.core_capabilities.first().cloned().unwrap_or_default()
                };

                let short_models: Vec<String> = yaml
                    .mental_models
                    .iter()
                    .map(|m| m.split(" - ").next().unwrap_or(m).trim().to_string())
                    .collect();

                let short_caps: Vec<String> = yaml
                    .core_capabilities
                    .iter()
                    .map(|c| {
                        c.split('.')
                            .next()
                            .unwrap_or(c)
                            .split(',')
                            .next()
                            .unwrap_or(c)
                            .trim()
                            .to_string()
                    })
                    .take(4)
                    .collect();

                personas.push(PersonaInfo {
                    id: yaml.id,
                    name: yaml.name,
                    role: yaml.role,
                    expertise,
                    mental_models: short_models,
                    core_capabilities: short_caps,
                    enabled: true,
                    file_path: Some(path.display().to_string()),
                    tags: yaml.recommended_skills.clone(),
                });
            }
        }
    }

    if personas.is_empty() { None } else { Some(personas) }
}

pub(crate) fn fallback_personas() -> Vec<PersonaInfo> {
    let (enabled, file_path, tags) = super::default_lib_fields();
    vec![
        PersonaInfo { id: "jeff-bezos".into(), name: "Jeff Bezos".into(), role: "ceo".into(), expertise: "Customer-obsessed leader. Uses PR/FAQ, flywheel thinking, Day 1 mindset.".into(), mental_models: vec!["PR/FAQ".into(), "Flywheel Effect".into(), "Day 1 Mindset".into()], core_capabilities: vec!["Strategic decisions".into(), "Resource allocation".into()], enabled, file_path: file_path.clone(), tags: tags.clone() },
        PersonaInfo { id: "dhh".into(), name: "David Heinemeier Hansson".into(), role: "fullstack".into(), expertise: "Creator of Ruby on Rails. Pragmatic, opinionated developer.".into(), mental_models: vec!["Convention over Configuration".into(), "Majestic Monolith".into()], core_capabilities: vec!["Full-stack development".into(), "Architecture decisions".into()], enabled, file_path: file_path.clone(), tags: tags.clone() },
        PersonaInfo { id: "kelsey-hightower".into(), name: "Kelsey Hightower".into(), role: "devops".into(), expertise: "Cloud-native expert. Kubernetes, infrastructure as code.".into(), mental_models: vec!["Infrastructure as Code".into(), "12-Factor App".into()], core_capabilities: vec!["DevOps pipelines".into(), "Cloud deployment".into()], enabled, file_path: file_path.clone(), tags: tags.clone() },
        PersonaInfo { id: "charlie-munger".into(), name: "Charlie Munger".into(), role: "critic".into(), expertise: "Inversion thinking, mental models, finding flaws.".into(), mental_models: vec!["Inversion".into(), "Second-Order Thinking".into()], core_capabilities: vec!["Risk assessment".into(), "Pre-mortem analysis".into()], enabled, file_path: file_path.clone(), tags: tags.clone() },
        PersonaInfo { id: "don-norman".into(), name: "Don Norman".into(), role: "product".into(), expertise: "Father of UX design. Human-centered design, usability.".into(), mental_models: vec!["Human-Centered Design".into(), "Affordances".into()], core_capabilities: vec!["User research".into(), "Product strategy".into()], enabled, file_path: file_path.clone(), tags: tags.clone() },
        PersonaInfo { id: "matias-duarte".into(), name: "Matias Duarte".into(), role: "ui".into(), expertise: "Material Design creator. Visual systems thinker.".into(), mental_models: vec!["Material Design".into(), "Design Systems".into()], core_capabilities: vec!["UI design".into(), "Design systems".into()], enabled, file_path: file_path.clone(), tags: tags.clone() },
        PersonaInfo { id: "james-bach".into(), name: "James Bach".into(), role: "qa".into(), expertise: "Exploratory testing pioneer.".into(), mental_models: vec!["Exploratory Testing".into(), "Risk-Based Testing".into()], core_capabilities: vec!["Test strategy".into(), "Bug hunting".into()], enabled, file_path: file_path.clone(), tags: tags.clone() },
        PersonaInfo { id: "seth-godin".into(), name: "Seth Godin".into(), role: "marketing".into(), expertise: "Permission marketing, Purple Cow, Tribes.".into(), mental_models: vec!["Purple Cow".into(), "Permission Marketing".into()], core_capabilities: vec!["Brand strategy".into(), "Content marketing".into()], enabled, file_path: file_path.clone(), tags: tags.clone() },
        PersonaInfo { id: "paul-graham".into(), name: "Paul Graham".into(), role: "operations".into(), expertise: "Y Combinator founder. Do things that don't scale.".into(), mental_models: vec!["Do Things That Don't Scale".into(), "Ramen Profitability".into()], core_capabilities: vec!["Startup operations".into(), "Product-market fit".into()], enabled, file_path: file_path.clone(), tags: tags.clone() },
        PersonaInfo { id: "aaron-ross".into(), name: "Aaron Ross".into(), role: "sales".into(), expertise: "Predictable Revenue author.".into(), mental_models: vec!["Predictable Revenue".into(), "Sales Assembly Line".into()], core_capabilities: vec!["Sales strategy".into(), "Pipeline building".into()], enabled, file_path: file_path.clone(), tags: tags.clone() },
        PersonaInfo { id: "patrick-campbell".into(), name: "Patrick Campbell".into(), role: "cfo".into(), expertise: "ProfitWell founder. SaaS metrics, pricing strategy.".into(), mental_models: vec!["Unit Economics".into(), "Value-Based Pricing".into()], core_capabilities: vec!["Financial modeling".into(), "Pricing strategy".into()], enabled, file_path: file_path.clone(), tags: tags.clone() },
        PersonaInfo { id: "ben-thompson".into(), name: "Ben Thompson".into(), role: "research".into(), expertise: "Stratechery author. Aggregation theory, platform dynamics.".into(), mental_models: vec!["Aggregation Theory".into(), "Platform Dynamics".into()], core_capabilities: vec!["Market research".into(), "Competitive analysis".into()], enabled, file_path, tags },
    ]
}
