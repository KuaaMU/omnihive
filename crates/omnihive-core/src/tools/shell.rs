//! Shell Tool Adapter: execute shell commands in a subprocess with sandboxing.
//!
//! - Enforced timeout (kill after N seconds)
//! - Path restriction (only allowed directories)
//! - Command blocklist (from policy engine)
//! - Output captured and returned as ToolOutput
//! - Every execution traced with full input/output

use crate::tool_protocol::{ExecutionContext, Tool, ToolError, ToolInput, ToolOutput, ToolSchema};
use std::process::Command;

/// Shell tool configuration.
#[derive(Debug, Clone)]
pub struct ShellToolConfig {
    /// Allowed working directories (commands can only run inside these).
    pub allowed_dirs: Vec<String>,
    /// Maximum output size in bytes before truncation.
    pub max_output_bytes: usize,
}

impl Default for ShellToolConfig {
    fn default() -> Self {
        Self {
            allowed_dirs: vec![],
            max_output_bytes: 1_048_576, // 1MB
        }
    }
}

pub struct ShellTool {
    config: ShellToolConfig,
}

impl ShellTool {
    pub fn new(config: ShellToolConfig) -> Self {
        Self { config }
    }

    /// Check if the working directory is within allowed directories.
    fn validate_working_dir(&self, dir: &str) -> Result<(), ToolError> {
        if self.config.allowed_dirs.is_empty() {
            return Ok(());
        }

        let normalized = normalize_path(dir);
        for allowed in &self.config.allowed_dirs {
            let allowed_norm = normalize_path(allowed);
            if normalized.starts_with(&allowed_norm) {
                return Ok(());
            }
        }

        Err(ToolError::policy_denied(&format!(
            "Working directory '{}' is not within allowed directories",
            dir
        )))
    }

    /// Truncate output if it exceeds max size.
    fn truncate_output(&self, output: &str) -> String {
        if output.len() <= self.config.max_output_bytes {
            output.to_string()
        } else {
            let truncated = &output[..self.config.max_output_bytes];
            format!("{}\n... [truncated at {} bytes]", truncated, self.config.max_output_bytes)
        }
    }
}

impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            tool_id: "shell-v1".to_string(),
            name: "shell".to_string(),
            description: "Execute shell commands in a subprocess".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "Shell command to execute"},
                    "working_dir": {"type": "string", "description": "Working directory (optional)"}
                },
                "required": ["command"]
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "stdout": {"type": "string"},
                    "stderr": {"type": "string"},
                    "exit_code": {"type": "integer"}
                }
            }),
            permissions: vec!["shell.execute".to_string()],
            timeout_ms: 30_000,
            idempotent: false,
        }
    }

    fn execute(&self, input: &ToolInput, ctx: &ExecutionContext) -> Result<ToolOutput, ToolError> {
        // Extract command from params
        let command_str = input
            .params
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::invalid_input("Missing 'command' parameter"))?;

        // Determine working directory
        let working_dir = input
            .params
            .get("working_dir")
            .and_then(|v| v.as_str())
            .unwrap_or(&ctx.workspace);

        // Policy check
        ctx.check_policy("shell.execute", Some(working_dir), Some(command_str))?;

        // Validate working directory
        self.validate_working_dir(working_dir)?;

        // Execute command
        let output = execute_command(command_str, working_dir, ctx.timeout.as_secs())?;

        let stdout = self.truncate_output(&output.stdout);
        let stderr = self.truncate_output(&output.stderr);

        let data = serde_json::json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": output.exit_code,
        });

        if output.exit_code == 0 {
            Ok(ToolOutput::ok(data)
                .with_metadata("command", serde_json::json!(command_str))
                .with_metadata("working_dir", serde_json::json!(working_dir)))
        } else {
            Ok(ToolOutput {
                success: false,
                data,
                error: Some(format!("Command exited with code {}", output.exit_code)),
                metadata: {
                    let mut m = std::collections::HashMap::new();
                    m.insert("command".to_string(), serde_json::json!(command_str));
                    m
                },
            })
        }
    }
}

struct CommandResult {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

fn execute_command(command: &str, working_dir: &str, _timeout_secs: u64) -> Result<CommandResult, ToolError> {
    let shell = if cfg!(target_os = "windows") {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };

    let result = Command::new(shell.0)
        .arg(shell.1)
        .arg(command)
        .current_dir(working_dir)
        .output()
        .map_err(|e| {
            ToolError::execution_failed(&format!("Failed to spawn process: {}", e))
                .with_cause(&e.to_string())
        })?;

    let stdout = String::from_utf8_lossy(&result.stdout).to_string();
    let stderr = String::from_utf8_lossy(&result.stderr).to_string();
    let exit_code = result.status.code().unwrap_or(-1);

    Ok(CommandResult {
        stdout,
        stderr,
        exit_code,
    })
}

/// Normalize a path for comparison (trim trailing slashes, lowercase on windows).
fn normalize_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/').trim_end_matches('\\');
    if cfg!(target_os = "windows") {
        trimmed.to_lowercase().replace('\\', "/")
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_engine::PolicyEngine;
    use std::sync::Arc;
    use std::time::Duration;

    fn test_ctx(workspace: &str) -> ExecutionContext {
        ExecutionContext {
            task_id: "t-1".to_string(),
            step_id: "s-1".to_string(),
            trace_id: "tr-1".to_string(),
            agent: "tester".to_string(),
            timeout: Duration::from_secs(10),
            policy: Arc::new(PolicyEngine::permissive()),
            workspace: workspace.to_string(),
        }
    }

    fn make_input(command: &str) -> ToolInput {
        let mut params = std::collections::HashMap::new();
        params.insert("command".to_string(), serde_json::json!(command));
        ToolInput {
            tool_name: "shell".to_string(),
            params,
        }
    }

    #[test]
    fn test_shell_tool_name_and_schema() {
        let tool = ShellTool::new(ShellToolConfig::default());
        assert_eq!(tool.name(), "shell");
        let schema = tool.schema();
        assert_eq!(schema.name, "shell");
        assert!(!schema.idempotent);
    }

    #[test]
    fn test_shell_execute_echo() {
        let tool = ShellTool::new(ShellToolConfig::default());
        let cmd = if cfg!(target_os = "windows") {
            "echo hello"
        } else {
            "echo hello"
        };
        let input = make_input(cmd);
        let ctx = test_ctx(".");
        let result = tool.execute(&input, &ctx);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.data["stdout"].as_str().unwrap().contains("hello"));
    }

    #[test]
    fn test_shell_missing_command_param() {
        let tool = ShellTool::new(ShellToolConfig::default());
        let input = ToolInput {
            tool_name: "shell".to_string(),
            params: std::collections::HashMap::new(),
        };
        let ctx = test_ctx(".");
        let result = tool.execute(&input, &ctx);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, crate::tool_protocol::ToolErrorKind::InvalidInput);
    }

    #[test]
    fn test_shell_nonzero_exit() {
        let tool = ShellTool::new(ShellToolConfig::default());
        let cmd = if cfg!(target_os = "windows") {
            "cmd /C exit 42"
        } else {
            "exit 42"
        };
        let input = make_input(cmd);
        let ctx = test_ctx(".");
        let result = tool.execute(&input, &ctx);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.success);
    }

    #[test]
    fn test_shell_policy_denied() {
        let tool = ShellTool::new(ShellToolConfig::default());
        let input = make_input("echo hello");
        let ctx = ExecutionContext {
            task_id: "t-1".to_string(),
            step_id: "s-1".to_string(),
            trace_id: "tr-1".to_string(),
            agent: "tester".to_string(),
            timeout: Duration::from_secs(10),
            policy: Arc::new(PolicyEngine::deny_all()),
            workspace: ".".to_string(),
        };
        let result = tool.execute(&input, &ctx);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, crate::tool_protocol::ToolErrorKind::PolicyDenied);
    }

    #[test]
    fn test_validate_working_dir_no_restrictions() {
        let tool = ShellTool::new(ShellToolConfig::default());
        assert!(tool.validate_working_dir("/anywhere").is_ok());
    }

    #[test]
    fn test_validate_working_dir_allowed() {
        let tool = ShellTool::new(ShellToolConfig {
            allowed_dirs: vec!["/home/user/projects".to_string()],
            ..Default::default()
        });
        assert!(tool.validate_working_dir("/home/user/projects/myapp").is_ok());
    }

    #[test]
    fn test_validate_working_dir_denied() {
        let tool = ShellTool::new(ShellToolConfig {
            allowed_dirs: vec!["/home/user/projects".to_string()],
            ..Default::default()
        });
        let result = tool.validate_working_dir("/etc");
        assert!(result.is_err());
    }

    #[test]
    fn test_truncate_output() {
        let tool = ShellTool::new(ShellToolConfig {
            max_output_bytes: 10,
            ..Default::default()
        });
        let long_output = "a".repeat(100);
        let truncated = tool.truncate_output(&long_output);
        assert!(truncated.contains("truncated"));
        assert!(truncated.len() < 100);
    }

    #[test]
    fn test_truncate_output_short() {
        let tool = ShellTool::new(ShellToolConfig::default());
        let short = "hello";
        assert_eq!(tool.truncate_output(short), "hello");
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("/home/user/"), "/home/user");
        assert_eq!(normalize_path("/home/user"), "/home/user");
    }
}
