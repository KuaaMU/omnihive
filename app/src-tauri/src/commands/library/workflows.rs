use crate::models::WorkflowInfo;
use super::registry::get_library_dir;

#[derive(serde::Deserialize)]
struct WorkflowYaml {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    chain: Vec<WorkflowStepYaml>,
    #[serde(default = "default_convergence")]
    convergence_cycles: u32,
}

#[derive(serde::Deserialize)]
struct WorkflowStepYaml {
    role: String,
    #[allow(dead_code)]
    #[serde(default)]
    persona: String,
}

fn default_convergence() -> u32 { 1 }

pub(crate) fn load_workflows_from_files() -> Option<Vec<WorkflowInfo>> {
    let lib_dir = get_library_dir()?;
    let workflows_dir = lib_dir.join("workflows");
    if !workflows_dir.exists() {
        return None;
    }

    let mut workflows = Vec::new();
    let entries = std::fs::read_dir(&workflows_dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name()?.to_string_lossy().to_string();
        if !name.ends_with(".yaml") {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(yaml) = serde_yaml::from_str::<WorkflowYaml>(&content) {
                let chain: Vec<String> = yaml.chain.iter().map(|s| s.role.clone()).collect();
                workflows.push(WorkflowInfo {
                    id: yaml.id,
                    name: yaml.name,
                    description: yaml.description,
                    chain,
                    convergence_cycles: yaml.convergence_cycles,
                    enabled: true,
                    file_path: Some(path.display().to_string()),
                    tags: vec![],
                });
            }
        }
    }

    if workflows.is_empty() { None } else { Some(workflows) }
}

pub(crate) fn fallback_workflows() -> Vec<WorkflowInfo> {
    let (enabled, file_path, tags) = super::default_lib_fields();
    vec![
        WorkflowInfo { id: "pricing-monetization".into(), name: "Pricing & Monetization".into(), description: "End-to-end pricing strategy workflow.".into(), chain: vec!["research".into(), "cfo".into(), "product".into(), "marketing".into(), "critic".into(), "cfo".into()], convergence_cycles: 2, enabled, file_path: file_path.clone(), tags: tags.clone() },
        WorkflowInfo { id: "product-launch".into(), name: "Product Launch".into(), description: "Coordinated product launch workflow.".into(), chain: vec!["marketing".into(), "research".into(), "sales".into(), "marketing".into(), "devops".into(), "ceo".into()], convergence_cycles: 2, enabled, file_path: file_path.clone(), tags: tags.clone() },
        WorkflowInfo { id: "weekly-review".into(), name: "Weekly Review".into(), description: "Weekly strategic review cycle.".into(), chain: vec!["research".into(), "cfo".into(), "marketing".into(), "qa".into(), "ceo".into(), "critic".into()], convergence_cycles: 1, enabled, file_path: file_path.clone(), tags: tags.clone() },
        WorkflowInfo { id: "new-product-eval".into(), name: "New Product Evaluation".into(), description: "Evaluate new product ideas.".into(), chain: vec!["research".into(), "product".into(), "cfo".into(), "critic".into(), "ceo".into()], convergence_cycles: 2, enabled, file_path: file_path.clone(), tags: tags.clone() },
        WorkflowInfo { id: "feature-development".into(), name: "Feature Development".into(), description: "End-to-end feature development.".into(), chain: vec!["product".into(), "fullstack".into(), "qa".into(), "devops".into()], convergence_cycles: 1, enabled, file_path: file_path.clone(), tags: tags.clone() },
        WorkflowInfo { id: "opportunity-discovery".into(), name: "Opportunity Discovery".into(), description: "Discover and validate market opportunities.".into(), chain: vec!["research".into(), "marketing".into(), "sales".into(), "cfo".into(), "ceo".into()], convergence_cycles: 2, enabled, file_path, tags },
    ]
}
