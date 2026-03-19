use serde::{Deserialize, Serialize};

// ===== Enums =====

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ModelTier {
    Opus,
    Sonnet,
    Haiku,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Claude,
    Codex,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentLayer {
    Strategy,
    Engineering,
    Product,
    Business,
    Intelligence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Complexity {
    Simple,
    Medium,
    Complex,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Initializing,
    Running,
    Paused,
    Stopped,
    Error,
}

// ===== Config Structs =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaRef {
    pub id: String,
    #[serde(default)]
    pub custom_instructions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub role: String,
    pub persona: PersonaRef,
    #[serde(default)]
    pub skills: Vec<String>,
    pub model: ModelTier,
    pub layer: AgentLayer,
    #[serde(default)]
    pub decides: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    pub id: String,
    pub name: String,
    pub description: String,
    pub chain: Vec<String>,
    pub convergence_cycles: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub engine: Engine,
    pub model: ModelTier,
    #[serde(default)]
    pub api_key_env: String,
    #[serde(default)]
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub max_daily_usd: f64,
    pub alert_at_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub providers: Vec<ProviderConfig>,
    #[serde(default = "default_failover")]
    pub failover: String,
    pub budget: BudgetConfig,
    #[serde(default = "default_loop_interval")]
    pub loop_interval: u32,
    #[serde(default = "default_cycle_timeout")]
    pub cycle_timeout: u32,
    #[serde(default = "default_max_errors")]
    pub max_consecutive_errors: u32,
}

fn default_failover() -> String { "auto".to_string() }
fn default_loop_interval() -> u32 { 30 }
fn default_cycle_timeout() -> u32 { 1800 }
fn default_max_errors() -> u32 { 5 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailConfig {
    pub forbidden: Vec<String>,
    #[serde(default = "default_workspace")]
    pub workspace: String,
    #[serde(default)]
    pub require_critic_review: bool,
}

fn default_workspace() -> String { "projects/".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyConfig {
    pub name: String,
    pub mission: String,
    #[serde(default)]
    pub description: String,
    pub seed_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgConfig {
    pub agents: Vec<AgentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryConfig {
    pub company: CompanyConfig,
    pub org: OrgConfig,
    pub workflows: Vec<WorkflowConfig>,
    pub runtime: RuntimeConfig,
    pub guardrails: GuardrailConfig,
}

// ===== Seed Analysis =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedAnalysis {
    pub domain: String,
    pub audience: String,
    pub complexity: Complexity,
    pub features: Vec<String>,
    pub suggested_roles: Vec<String>,
    pub team_size: usize,
}

// ===== Runtime State =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub is_running: bool,
    pub pid: Option<u32>,
    pub current_cycle: u32,
    pub total_cycles: u32,
    pub consecutive_errors: u32,
    pub last_cycle_at: Option<String>,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleResult {
    pub cycle_number: u32,
    pub started_at: String,
    pub completed_at: String,
    pub agent_role: String,
    pub action: String,
    pub outcome: String,
    pub files_changed: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusState {
    pub company_name: String,
    pub mission: String,
    pub status: ProjectStatus,
    pub cycle: u32,
    pub revenue: String,
    pub current_focus: String,
    pub active_projects: Vec<String>,
    pub next_action: String,
    pub raw_content: String,
}

// ===== Library =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaInfo {
    pub id: String,
    pub name: String,
    pub role: String,
    pub expertise: String,
    pub mental_models: Vec<String>,
    pub core_capabilities: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub source: String,
    pub content_preview: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub chain: Vec<String>,
    pub convergence_cycles: u32,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

// ===== Project =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub seed_prompt: String,
    pub output_dir: String,
    pub created_at: String,
    pub last_active_at: String,
    pub status: ProjectStatus,
    pub agent_count: usize,
    pub cycle_count: u32,
}

// ===== Generate Result =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResult {
    pub output_dir: String,
    pub files_created: Vec<String>,
    pub agent_count: usize,
    pub skill_count: usize,
    pub workflow_count: usize,
}

// ===== Skill Repository =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRepo {
    pub id: String,
    pub name: String,
    pub owner: String,           // GitHub owner, e.g. "anthropics"
    pub repo: String,            // GitHub repo name, e.g. "skills"
    pub branch: String,          // branch name, e.g. "main"
    pub path: String,            // subdirectory within repo, e.g. "skills"
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoItem {
    pub name: String,
    pub path: String,
    pub item_type: String,       // "file" or "dir"
    pub download_url: Option<String>,
    pub description: String,
}

// ===== App Settings =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub default_engine: String,
    pub default_model: String,
    pub max_daily_budget: f64,
    pub alert_at_budget: f64,
    pub loop_interval: u32,
    pub cycle_timeout: u32,
    pub projects_dir: String,
    pub providers: Vec<AiProvider>,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
    #[serde(default)]
    pub skill_repos: Vec<SkillRepo>,
}

fn default_language() -> String { "en".to_string() }
fn default_theme() -> String { "obsidian".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProvider {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub api_key: String,
    pub api_base_url: String,
    pub default_model: String,
    pub enabled: bool,
    pub is_healthy: bool,
    pub last_error: Option<String>,
    #[serde(default = "default_provider_engine")]
    pub engine: String,
    #[serde(default = "default_anthropic_version")]
    pub anthropic_version: String,
    #[serde(default)]
    pub extra_headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub force_stream: bool,
    #[serde(default = "default_api_format")]
    pub api_format: String,
}

fn default_provider_engine() -> String { "claude".to_string() }
fn default_anthropic_version() -> String { "2023-06-01".to_string() }
fn default_api_format() -> String { "anthropic".to_string() }

// ===== Resolved Runtime Config (for config preview) =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedRuntimeConfig {
    pub engine: String,
    pub model_tier: String,
    pub resolved_model: String,
    pub provider_name: String,
    pub provider_type: String,
    pub api_base_url: String,
    pub api_key_preview: String,
    pub source: String,
}

// ===== MCP Server Configuration =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub server_type: String,        // "stdio" | "sse" | "streamable-http"
    pub command: String,             // for stdio: executable command
    pub args: Vec<String>,           // for stdio: command arguments
    pub url: String,                 // for sse/http: server URL
    pub env: std::collections::HashMap<String, String>,  // environment variables
    pub enabled: bool,
    pub tools: Vec<McpToolInfo>,     // discovered tools (cached)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
}

// ===== Project Registry =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRegistryEntry {
    pub id: String,
    pub name: String,
    pub output_dir: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectRegistry {
    pub projects: Vec<ProjectRegistryEntry>,
}

// ===== Log Event =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub agent: String,
    pub message: String,
}

// ===== Per-Project Runtime Override =====

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectRuntimeOverride {
    #[serde(default)]
    pub engine: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
}

// ===== Project Event (Activity Feed) =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEvent {
    pub id: String,
    pub timestamp: String,
    pub event_type: String,
    pub agent: String,
    pub summary: String,
    #[serde(default)]
    pub details: String,
}

// ===== Detected Provider (auto-detection) =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedProvider {
    pub source: String,
    pub provider_type: String,
    pub api_key_preview: String,
    pub api_key: String,
    pub api_base_url: String,
    pub suggested_name: String,
    pub suggested_model: String,
}

// ===== System Environment =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub default_shell: String,
    pub shells: Vec<ShellInfo>,
    pub tools: Vec<ToolInfo>,
    pub node_version: Option<String>,
    pub npm_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellInfo {
    pub name: String,
    pub path: Option<String>,
    pub version: Option<String>,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub display_name: String,
    pub available: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub install_command: String,
    pub install_url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cycle_result_serde_roundtrip() {
        let original = CycleResult {
            cycle_number: 5,
            started_at: "2025-01-01T00:00:00+00:00".to_string(),
            completed_at: "2025-01-01T00:01:00+00:00".to_string(),
            agent_role: "ceo".to_string(),
            action: "Strategic analysis".to_string(),
            outcome: "Decided to pivot".to_string(),
            files_changed: vec!["consensus.md".to_string()],
            error: None,
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: CycleResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.cycle_number, 5);
        assert_eq!(parsed.agent_role, "ceo");
        assert!(parsed.error.is_none());
    }

    #[test]
    fn test_cycle_result_with_error() {
        let original = CycleResult {
            cycle_number: 3,
            started_at: "t".to_string(),
            completed_at: "t".to_string(),
            agent_role: "devops".to_string(),
            action: "deploy".to_string(),
            outcome: String::new(),
            files_changed: vec![],
            error: Some("API timeout".to_string()),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: CycleResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.error, Some("API timeout".to_string()));
    }

    #[test]
    fn test_runtime_status_serde_roundtrip() {
        let original = RuntimeStatus {
            is_running: true,
            pid: Some(1234),
            current_cycle: 10,
            total_cycles: 50,
            consecutive_errors: 0,
            last_cycle_at: Some("2025-01-01T00:00:00+00:00".to_string()),
            uptime_seconds: 3600,
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: RuntimeStatus = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_running);
        assert_eq!(parsed.pid, Some(1234));
        assert_eq!(parsed.current_cycle, 10);
    }

    #[test]
    fn test_project_status_enum_serde() {
        let statuses = vec![
            ProjectStatus::Initializing,
            ProjectStatus::Running,
            ProjectStatus::Paused,
            ProjectStatus::Stopped,
            ProjectStatus::Error,
        ];
        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: ProjectStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn test_guardrail_config_serde() {
        let config = GuardrailConfig {
            forbidden: vec!["rm -rf /".to_string(), "gh repo delete".to_string()],
            workspace: "projects/".to_string(),
            require_critic_review: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: GuardrailConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.forbidden.len(), 2);
        assert!(parsed.require_critic_review);
    }

    #[test]
    fn test_guardrail_config_defaults() {
        let json = r#"{"forbidden":[]}"#;
        let parsed: GuardrailConfig = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.workspace, "projects/");
        assert!(!parsed.require_critic_review);
    }

    #[test]
    fn test_project_runtime_override_default() {
        let ovr = ProjectRuntimeOverride::default();
        assert!(ovr.engine.is_none());
        assert!(ovr.model.is_none());
        assert!(ovr.provider_id.is_none());
    }

    #[test]
    fn test_project_runtime_override_serde() {
        let ovr = ProjectRuntimeOverride {
            engine: Some("claude".to_string()),
            model: None,
            provider_id: Some("p1".to_string()),
        };
        let json = serde_json::to_string(&ovr).unwrap();
        let parsed: ProjectRuntimeOverride = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.engine, Some("claude".to_string()));
        assert!(parsed.model.is_none());
    }

    #[test]
    fn test_factory_config_yaml_roundtrip() {
        let yaml = r#"
company:
  name: "Test Corp"
  mission: "Test mission"
  seed_prompt: "Build a test app"
org:
  agents:
    - role: ceo
      persona:
        id: jeff-bezos
      model: opus
      layer: strategy
workflows:
  - id: test-flow
    name: Test Flow
    description: A test workflow
    chain: [ceo]
    convergence_cycles: 1
runtime:
  providers:
    - engine: claude
      model: sonnet
  budget:
    max_daily_usd: 10.0
    alert_at_usd: 8.0
guardrails:
  forbidden:
    - "rm -rf /"
"#;
        let config: FactoryConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.company.name, "Test Corp");
        assert_eq!(config.org.agents.len(), 1);
        assert_eq!(config.org.agents[0].role, "ceo");
        assert_eq!(config.workflows.len(), 1);
        assert_eq!(config.guardrails.forbidden.len(), 1);
    }

    #[test]
    fn test_model_tier_enum() {
        let json = r#""opus""#;
        let tier: ModelTier = serde_json::from_str(json).unwrap();
        assert_eq!(tier, ModelTier::Opus);
    }

    #[test]
    fn test_persona_info_serde() {
        let info = PersonaInfo {
            id: "test".to_string(),
            name: "Test".to_string(),
            role: "ceo".to_string(),
            expertise: "Testing".to_string(),
            mental_models: vec!["model1".to_string()],
            core_capabilities: vec!["cap1".to_string()],
            enabled: true,
            file_path: None,
            tags: vec![],
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: PersonaInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test");
        assert!(parsed.enabled);
    }
}
