//! Tool Protocol: unified trait and types for tool execution.
//!
//! All tool adapters (shell, filesystem, github, etc.) implement the `Tool` trait.
//! Every tool call passes through the policy engine and is traced via `ExecutionContext`.

use crate::policy_engine::{PolicyDecision, PolicyEngine, ToolRequest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// ===== Tool Schema =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub tool_id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub output_schema: serde_json::Value,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub idempotent: bool,
}

fn default_timeout_ms() -> u64 {
    30_000
}

// ===== Tool Input / Output =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInput {
    pub tool_name: String,
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub success: bool,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ToolOutput {
    pub fn ok(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data,
            error: None,
            metadata: HashMap::new(),
        }
    }

    pub fn err(message: &str) -> Self {
        Self {
            success: false,
            data: serde_json::Value::Null,
            error: Some(message.to_string()),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }
}

// ===== Tool Error =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolError {
    pub kind: ToolErrorKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorKind {
    PolicyDenied,
    Timeout,
    ExecutionFailed,
    InvalidInput,
    NotFound,
    PermissionDenied,
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl ToolError {
    pub fn policy_denied(reason: &str) -> Self {
        Self {
            kind: ToolErrorKind::PolicyDenied,
            message: reason.to_string(),
            cause: None,
        }
    }

    pub fn timeout(message: &str) -> Self {
        Self {
            kind: ToolErrorKind::Timeout,
            message: message.to_string(),
            cause: None,
        }
    }

    pub fn execution_failed(message: &str) -> Self {
        Self {
            kind: ToolErrorKind::ExecutionFailed,
            message: message.to_string(),
            cause: None,
        }
    }

    pub fn invalid_input(message: &str) -> Self {
        Self {
            kind: ToolErrorKind::InvalidInput,
            message: message.to_string(),
            cause: None,
        }
    }

    pub fn not_found(message: &str) -> Self {
        Self {
            kind: ToolErrorKind::NotFound,
            message: message.to_string(),
            cause: None,
        }
    }

    pub fn with_cause(mut self, cause: &str) -> Self {
        self.cause = Some(cause.to_string());
        self
    }
}

// ===== Execution Context =====

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub task_id: String,
    pub step_id: String,
    pub trace_id: String,
    pub agent: String,
    pub timeout: Duration,
    pub policy: Arc<PolicyEngine>,
    pub workspace: String,
}

impl ExecutionContext {
    pub fn new(
        task_id: &str,
        step_id: &str,
        trace_id: &str,
        agent: &str,
        policy: Arc<PolicyEngine>,
        workspace: &str,
    ) -> Self {
        Self {
            task_id: task_id.to_string(),
            step_id: step_id.to_string(),
            trace_id: trace_id.to_string(),
            agent: agent.to_string(),
            timeout: Duration::from_secs(30),
            policy,
            workspace: workspace.to_string(),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Check if an action is allowed by the policy engine.
    pub fn check_policy(&self, action: &str, path: Option<&str>, command: Option<&str>) -> Result<(), ToolError> {
        let request = ToolRequest {
            action: action.to_string(),
            path: path.map(|s| s.to_string()),
            command: command.map(|s| s.to_string()),
            agent: Some(self.agent.clone()),
        };

        match self.policy.evaluate(&request) {
            PolicyDecision::Allow => Ok(()),
            PolicyDecision::Deny { reason, .. } => Err(ToolError::policy_denied(&reason)),
            PolicyDecision::RequiresApproval { reason, .. } => {
                Err(ToolError::policy_denied(&format!("Requires approval: {}", reason)))
            }
        }
    }
}

// ===== Tool Trait =====

/// Trait that all tool adapters must implement.
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn schema(&self) -> ToolSchema;
    fn execute(&self, input: &ToolInput, ctx: &ExecutionContext) -> Result<ToolOutput, ToolError>;
}

// ===== Tool Registry =====

/// Registry of available tools, looked up by name.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.tools.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    pub fn execute(
        &self,
        input: &ToolInput,
        ctx: &ExecutionContext,
    ) -> Result<ToolOutput, ToolError> {
        let tool = self
            .tools
            .get(&input.tool_name)
            .ok_or_else(|| ToolError::not_found(&format!("Tool '{}' not found", input.tool_name)))?;

        tool.execute(input, ctx)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_engine::PolicyEngine;

    // --- Stub tool for testing ---

    struct EchoTool;

    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        fn schema(&self) -> ToolSchema {
            ToolSchema {
                tool_id: "echo-v1".to_string(),
                name: "echo".to_string(),
                description: "Echo back the input".to_string(),
                input_schema: serde_json::json!({"type": "object"}),
                output_schema: serde_json::json!({"type": "object"}),
                permissions: vec![],
                timeout_ms: 5000,
                idempotent: true,
            }
        }

        fn execute(&self, input: &ToolInput, _ctx: &ExecutionContext) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::ok(serde_json::to_value(&input.params).unwrap()))
        }
    }

    fn test_ctx() -> ExecutionContext {
        ExecutionContext::new(
            "t-1",
            "s-1",
            "tr-1",
            "tester",
            Arc::new(PolicyEngine::permissive()),
            "/tmp/workspace",
        )
    }

    fn deny_ctx() -> ExecutionContext {
        ExecutionContext::new(
            "t-1",
            "s-1",
            "tr-1",
            "tester",
            Arc::new(PolicyEngine::deny_all()),
            "/tmp/workspace",
        )
    }

    // --- ToolOutput ---

    #[test]
    fn test_tool_output_ok() {
        let out = ToolOutput::ok(serde_json::json!({"key": "value"}));
        assert!(out.success);
        assert!(out.error.is_none());
    }

    #[test]
    fn test_tool_output_err() {
        let out = ToolOutput::err("something failed");
        assert!(!out.success);
        assert_eq!(out.error.as_deref(), Some("something failed"));
    }

    #[test]
    fn test_tool_output_with_metadata() {
        let out = ToolOutput::ok(serde_json::json!(null))
            .with_metadata("duration_ms", serde_json::json!(42));
        assert_eq!(out.metadata["duration_ms"], 42);
    }

    // --- ToolError ---

    #[test]
    fn test_tool_error_policy_denied() {
        let err = ToolError::policy_denied("not allowed");
        assert_eq!(err.kind, ToolErrorKind::PolicyDenied);
        assert!(err.message.contains("not allowed"));
    }

    #[test]
    fn test_tool_error_with_cause() {
        let err = ToolError::execution_failed("cmd failed").with_cause("exit code 1");
        assert_eq!(err.cause.as_deref(), Some("exit code 1"));
    }

    #[test]
    fn test_tool_error_display() {
        let err = ToolError::timeout("took too long");
        let s = format!("{}", err);
        assert!(s.contains("Timeout"));
        assert!(s.contains("took too long"));
    }

    // --- ToolSchema serde ---

    #[test]
    fn test_tool_schema_serde_roundtrip() {
        let schema = ToolSchema {
            tool_id: "shell-v1".to_string(),
            name: "shell".to_string(),
            description: "Run shell commands".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: serde_json::json!({"type": "object"}),
            permissions: vec!["shell.execute".to_string()],
            timeout_ms: 60000,
            idempotent: false,
        };
        let json = serde_json::to_string(&schema).unwrap();
        let parsed: ToolSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tool_id, "shell-v1");
        assert_eq!(parsed.timeout_ms, 60000);
        assert!(!parsed.idempotent);
    }

    // --- ToolInput/ToolOutput serde ---

    #[test]
    fn test_tool_input_serde() {
        let input = ToolInput {
            tool_name: "shell".to_string(),
            params: {
                let mut m = HashMap::new();
                m.insert("command".to_string(), serde_json::json!("ls -la"));
                m
            },
        };
        let json = serde_json::to_string(&input).unwrap();
        let parsed: ToolInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tool_name, "shell");
        assert_eq!(parsed.params["command"], "ls -la");
    }

    // --- ExecutionContext ---

    #[test]
    fn test_execution_context_check_policy_allow() {
        let ctx = test_ctx();
        assert!(ctx.check_policy("shell.execute", None, Some("ls")).is_ok());
    }

    #[test]
    fn test_execution_context_check_policy_deny() {
        let ctx = deny_ctx();
        let result = ctx.check_policy("shell.execute", None, Some("ls"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ToolErrorKind::PolicyDenied);
    }

    #[test]
    fn test_execution_context_with_timeout() {
        let ctx = test_ctx().with_timeout(Duration::from_secs(120));
        assert_eq!(ctx.timeout, Duration::from_secs(120));
    }

    // --- ToolRegistry ---

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        assert!(registry.get("echo").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_list() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        let names = registry.list();
        assert_eq!(names, vec!["echo"]);
    }

    #[test]
    fn test_registry_execute() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));

        let input = ToolInput {
            tool_name: "echo".to_string(),
            params: {
                let mut m = HashMap::new();
                m.insert("msg".to_string(), serde_json::json!("hello"));
                m
            },
        };

        let result = registry.execute(&input, &test_ctx());
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success);
        assert_eq!(output.data["msg"], "hello");
    }

    #[test]
    fn test_registry_execute_not_found() {
        let registry = ToolRegistry::new();
        let input = ToolInput {
            tool_name: "missing".to_string(),
            params: HashMap::new(),
        };
        let result = registry.execute(&input, &test_ctx());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, ToolErrorKind::NotFound);
    }
}
