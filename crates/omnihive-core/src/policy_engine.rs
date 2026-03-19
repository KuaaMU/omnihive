//! Policy Engine: default-deny rule evaluation for tool calls and actions.
//!
//! Rules are evaluated in priority order. Deny rules win over allow rules at the
//! same priority. If no rule matches, the default is Deny.

use serde::{Deserialize, Serialize};

// ===== Policy Decision =====

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum PolicyDecision {
    Allow,
    Deny {
        reason: String,
        rule_id: String,
    },
    RequiresApproval {
        approver: String,
        reason: String,
    },
}

impl PolicyDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, PolicyDecision::Allow)
    }
}

// ===== Policy Rule =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub action: String,
    pub effect: RuleEffect,
    #[serde(default)]
    pub conditions: RuleConditions,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleEffect {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleConditions {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub agents: Vec<String>,
}

// ===== Tool Request =====

#[derive(Debug, Clone)]
pub struct ToolRequest {
    pub action: String,
    pub path: Option<String>,
    pub command: Option<String>,
    pub agent: Option<String>,
}

// ===== Policy Engine =====

#[derive(Debug)]
pub struct PolicyEngine {
    rules: Vec<PolicyRule>,
}

impl PolicyEngine {
    /// Create from a list of rules. Rules are sorted by priority (descending).
    pub fn new(mut rules: Vec<PolicyRule>) -> Self {
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        Self { rules }
    }

    /// Create with a permissive default policy (allow everything).
    pub fn permissive() -> Self {
        Self::new(vec![PolicyRule {
            action: "*".to_string(),
            effect: RuleEffect::Allow,
            conditions: RuleConditions::default(),
            priority: 0,
        }])
    }

    /// Create an empty engine (default-deny everything).
    pub fn deny_all() -> Self {
        Self::new(vec![])
    }

    /// Evaluate a tool request against the policy rules.
    ///
    /// Evaluation order:
    /// 1. Rules sorted by priority (highest first)
    /// 2. At same priority, deny wins over allow
    /// 3. First matching rule determines the outcome
    /// 4. No match = Deny (default-deny)
    pub fn evaluate(&self, request: &ToolRequest) -> PolicyDecision {
        for rule in &self.rules {
            if !action_matches(&rule.action, &request.action) {
                continue;
            }

            if !conditions_match(&rule.conditions, request) {
                continue;
            }

            return match rule.effect {
                RuleEffect::Allow => PolicyDecision::Allow,
                RuleEffect::Deny => PolicyDecision::Deny {
                    reason: format!(
                        "Action '{}' denied by rule matching '{}'",
                        request.action, rule.action
                    ),
                    rule_id: rule.action.clone(),
                },
            };
        }

        // Default deny
        PolicyDecision::Deny {
            reason: format!("No rule matched action '{}' (default deny)", request.action),
            rule_id: "default-deny".to_string(),
        }
    }

    /// Load policy rules from a project's guardrail configuration.
    pub fn from_guardrails(forbidden_commands: &[String], workspace: &str) -> Self {
        let mut rules = Vec::new();

        // High-priority deny rules for forbidden commands
        for cmd in forbidden_commands {
            rules.push(PolicyRule {
                action: "shell.execute".to_string(),
                effect: RuleEffect::Deny,
                conditions: RuleConditions {
                    commands: vec![cmd.clone()],
                    ..Default::default()
                },
                priority: 100,
            });
        }

        // Allow file operations within workspace
        if !workspace.is_empty() {
            rules.push(PolicyRule {
                action: "fs.*".to_string(),
                effect: RuleEffect::Allow,
                conditions: RuleConditions {
                    paths: vec![format!("{}*", workspace)],
                    ..Default::default()
                },
                priority: 10,
            });
        }

        // Allow API calls by default
        rules.push(PolicyRule {
            action: "api.call".to_string(),
            effect: RuleEffect::Allow,
            conditions: RuleConditions::default(),
            priority: 0,
        });

        Self::new(rules)
    }
}

// ===== Matching Helpers =====

fn action_matches(pattern: &str, action: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix(".*") {
        return action.starts_with(prefix);
    }
    pattern == action
}

fn conditions_match(conditions: &RuleConditions, request: &ToolRequest) -> bool {
    // If conditions specify commands, at least one must match
    if !conditions.commands.is_empty() {
        if let Some(ref cmd) = request.command {
            if !conditions.commands.iter().any(|c| cmd.contains(c)) {
                return false;
            }
        } else {
            return false;
        }
    }

    // If conditions specify paths, at least one must match
    if !conditions.paths.is_empty() {
        if let Some(ref path) = request.path {
            if !conditions.paths.iter().any(|p| path_matches(p, path)) {
                return false;
            }
        } else {
            return false;
        }
    }

    // If conditions specify agents, the requesting agent must be in the list
    if !conditions.agents.is_empty() {
        if let Some(ref agent) = request.agent {
            if !conditions.agents.iter().any(|a| a == agent) {
                return false;
            }
        } else {
            return false;
        }
    }

    true
}

fn path_matches(pattern: &str, path: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        return path.starts_with(prefix);
    }
    pattern == path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(action: &str) -> ToolRequest {
        ToolRequest {
            action: action.to_string(),
            path: None,
            command: None,
            agent: None,
        }
    }

    fn req_cmd(action: &str, cmd: &str) -> ToolRequest {
        ToolRequest {
            action: action.to_string(),
            path: None,
            command: Some(cmd.to_string()),
            agent: None,
        }
    }

    fn req_path(action: &str, path: &str) -> ToolRequest {
        ToolRequest {
            action: action.to_string(),
            path: Some(path.to_string()),
            command: None,
            agent: None,
        }
    }

    fn req_agent(action: &str, agent: &str) -> ToolRequest {
        ToolRequest {
            action: action.to_string(),
            path: None,
            command: None,
            agent: Some(agent.to_string()),
        }
    }

    // --- Default deny ---

    #[test]
    fn test_empty_engine_denies_everything() {
        let engine = PolicyEngine::deny_all();
        let decision = engine.evaluate(&req("shell.execute"));
        assert!(!decision.is_allowed());
        match decision {
            PolicyDecision::Deny { rule_id, .. } => assert_eq!(rule_id, "default-deny"),
            _ => panic!("Expected Deny"),
        }
    }

    // --- Permissive ---

    #[test]
    fn test_permissive_allows_everything() {
        let engine = PolicyEngine::permissive();
        assert!(engine.evaluate(&req("shell.execute")).is_allowed());
        assert!(engine.evaluate(&req("fs.read")).is_allowed());
        assert!(engine.evaluate(&req("anything")).is_allowed());
    }

    // --- Action matching ---

    #[test]
    fn test_exact_action_match() {
        let engine = PolicyEngine::new(vec![PolicyRule {
            action: "fs.read".to_string(),
            effect: RuleEffect::Allow,
            conditions: RuleConditions::default(),
            priority: 0,
        }]);
        assert!(engine.evaluate(&req("fs.read")).is_allowed());
        assert!(!engine.evaluate(&req("fs.write")).is_allowed());
    }

    #[test]
    fn test_wildcard_action_match() {
        let engine = PolicyEngine::new(vec![PolicyRule {
            action: "fs.*".to_string(),
            effect: RuleEffect::Allow,
            conditions: RuleConditions::default(),
            priority: 0,
        }]);
        assert!(engine.evaluate(&req("fs.read")).is_allowed());
        assert!(engine.evaluate(&req("fs.write")).is_allowed());
        assert!(!engine.evaluate(&req("shell.execute")).is_allowed());
    }

    // --- Priority: deny wins at same level ---

    #[test]
    fn test_deny_overrides_allow_at_higher_priority() {
        let engine = PolicyEngine::new(vec![
            PolicyRule {
                action: "shell.execute".to_string(),
                effect: RuleEffect::Allow,
                conditions: RuleConditions::default(),
                priority: 0,
            },
            PolicyRule {
                action: "shell.execute".to_string(),
                effect: RuleEffect::Deny,
                conditions: RuleConditions {
                    commands: vec!["rm -rf /".to_string()],
                    ..Default::default()
                },
                priority: 100,
            },
        ]);

        // Denied because the high-priority deny rule matches
        let decision = engine.evaluate(&req_cmd("shell.execute", "rm -rf /"));
        assert!(!decision.is_allowed());

        // Allowed because the deny rule's condition doesn't match
        let decision = engine.evaluate(&req_cmd("shell.execute", "ls -la"));
        assert!(decision.is_allowed());
    }

    // --- Command conditions ---

    #[test]
    fn test_command_condition_match() {
        let engine = PolicyEngine::new(vec![PolicyRule {
            action: "shell.execute".to_string(),
            effect: RuleEffect::Deny,
            conditions: RuleConditions {
                commands: vec!["git push --force".to_string()],
                ..Default::default()
            },
            priority: 0,
        }]);

        assert!(!engine.evaluate(&req_cmd("shell.execute", "git push --force main")).is_allowed());
        // No command in request = condition doesn't match = falls through to default deny
        assert!(!engine.evaluate(&req("shell.execute")).is_allowed());
    }

    // --- Path conditions ---

    #[test]
    fn test_path_condition_match() {
        let engine = PolicyEngine::new(vec![PolicyRule {
            action: "fs.write".to_string(),
            effect: RuleEffect::Allow,
            conditions: RuleConditions {
                paths: vec!["projects/*".to_string()],
                ..Default::default()
            },
            priority: 0,
        }]);

        assert!(engine.evaluate(&req_path("fs.write", "projects/myapp/src/main.rs")).is_allowed());
        assert!(!engine.evaluate(&req_path("fs.write", "/etc/passwd")).is_allowed());
    }

    // --- Agent conditions ---

    #[test]
    fn test_agent_condition_match() {
        let engine = PolicyEngine::new(vec![PolicyRule {
            action: "shell.execute".to_string(),
            effect: RuleEffect::Allow,
            conditions: RuleConditions {
                agents: vec!["devops".to_string()],
                ..Default::default()
            },
            priority: 0,
        }]);

        assert!(engine.evaluate(&req_agent("shell.execute", "devops")).is_allowed());
        assert!(!engine.evaluate(&req_agent("shell.execute", "ceo")).is_allowed());
    }

    // --- from_guardrails ---

    #[test]
    fn test_from_guardrails_blocks_forbidden() {
        let engine = PolicyEngine::from_guardrails(
            &["rm -rf /".to_string(), "git push --force main".to_string()],
            "projects/",
        );

        let decision = engine.evaluate(&req_cmd("shell.execute", "rm -rf /"));
        assert!(!decision.is_allowed());

        let decision = engine.evaluate(&req_cmd("shell.execute", "git push --force main"));
        assert!(!decision.is_allowed());
    }

    #[test]
    fn test_from_guardrails_allows_api() {
        let engine = PolicyEngine::from_guardrails(&[], "projects/");
        assert!(engine.evaluate(&req("api.call")).is_allowed());
    }

    #[test]
    fn test_from_guardrails_allows_fs_in_workspace() {
        let engine = PolicyEngine::from_guardrails(&[], "projects/");
        assert!(engine.evaluate(&req_path("fs.read", "projects/myapp/file.txt")).is_allowed());
        assert!(engine.evaluate(&req_path("fs.write", "projects/myapp/file.txt")).is_allowed());
    }

    // --- PolicyDecision serde ---

    #[test]
    fn test_policy_decision_serde_allow() {
        let d = PolicyDecision::Allow;
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("allow"));
    }

    #[test]
    fn test_policy_decision_serde_deny() {
        let d = PolicyDecision::Deny {
            reason: "blocked".to_string(),
            rule_id: "r1".to_string(),
        };
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("deny"));
        assert!(json.contains("blocked"));
    }

    // --- action_matches ---

    #[test]
    fn test_action_matches_star() {
        assert!(action_matches("*", "anything"));
    }

    #[test]
    fn test_action_matches_prefix_wildcard() {
        assert!(action_matches("fs.*", "fs.read"));
        assert!(action_matches("fs.*", "fs.write"));
        assert!(!action_matches("fs.*", "shell.execute"));
    }

    #[test]
    fn test_action_matches_exact() {
        assert!(action_matches("fs.read", "fs.read"));
        assert!(!action_matches("fs.read", "fs.write"));
    }

    // --- path_matches ---

    #[test]
    fn test_path_matches_wildcard() {
        assert!(path_matches("projects/*", "projects/a/b.txt"));
        assert!(!path_matches("projects/*", "/etc/passwd"));
    }

    #[test]
    fn test_path_matches_exact() {
        assert!(path_matches("/etc/hosts", "/etc/hosts"));
        assert!(!path_matches("/etc/hosts", "/etc/passwd"));
    }
}
