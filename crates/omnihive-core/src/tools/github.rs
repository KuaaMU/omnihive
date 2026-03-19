//! GitHub Tool Adapter: interact with GitHub via the `gh` CLI.
//!
//! Operations: list_issues, create_branch, commit_files, create_pr.
//! All operations audited in trace. Token sourced from environment.

use crate::tool_protocol::{ExecutionContext, Tool, ToolError, ToolInput, ToolOutput, ToolSchema};
use std::process::Command;

pub struct GitHubTool;

impl GitHubTool {
    pub fn new() -> Self {
        Self
    }

    fn run_gh(args: &[&str], working_dir: &str) -> Result<(String, String, i32), ToolError> {
        let result = Command::new("gh")
            .args(args)
            .current_dir(working_dir)
            .output()
            .map_err(|e| {
                ToolError::execution_failed(&format!(
                    "Failed to run gh CLI (is it installed?): {}",
                    e
                ))
            })?;

        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();
        let exit_code = result.status.code().unwrap_or(-1);

        Ok((stdout, stderr, exit_code))
    }

    fn run_git(args: &[&str], working_dir: &str) -> Result<(String, String, i32), ToolError> {
        let result = Command::new("git")
            .args(args)
            .current_dir(working_dir)
            .output()
            .map_err(|e| {
                ToolError::execution_failed(&format!("Failed to run git: {}", e))
            })?;

        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();
        let exit_code = result.status.code().unwrap_or(-1);

        Ok((stdout, stderr, exit_code))
    }

    fn execute_list_issues(ctx: &ExecutionContext) -> Result<ToolOutput, ToolError> {
        ctx.check_policy("github.read", None, None)?;

        let (stdout, stderr, exit_code) =
            Self::run_gh(&["issue", "list", "--json", "number,title,state,labels"], &ctx.workspace)?;

        if exit_code != 0 {
            return Err(ToolError::execution_failed(&format!(
                "gh issue list failed: {}",
                stderr
            )));
        }

        let issues: serde_json::Value =
            serde_json::from_str(&stdout).unwrap_or(serde_json::json!([]));

        Ok(ToolOutput::ok(serde_json::json!({
            "issues": issues,
            "count": issues.as_array().map(|a| a.len()).unwrap_or(0),
        })))
    }

    fn execute_create_branch(
        branch_name: &str,
        ctx: &ExecutionContext,
    ) -> Result<ToolOutput, ToolError> {
        ctx.check_policy("github.write", None, Some(&format!("git checkout -b {}", branch_name)))?;

        let (_, stderr, exit_code) =
            Self::run_git(&["checkout", "-b", branch_name], &ctx.workspace)?;

        if exit_code != 0 {
            return Err(ToolError::execution_failed(&format!(
                "git checkout -b failed: {}",
                stderr
            )));
        }

        Ok(ToolOutput::ok(serde_json::json!({
            "branch": branch_name,
            "created": true,
        })))
    }

    fn execute_commit(
        message: &str,
        files: &[&str],
        ctx: &ExecutionContext,
    ) -> Result<ToolOutput, ToolError> {
        ctx.check_policy("github.write", None, Some("git commit"))?;

        // Stage files
        for file in files {
            let (_, stderr, exit_code) = Self::run_git(&["add", file], &ctx.workspace)?;
            if exit_code != 0 {
                return Err(ToolError::execution_failed(&format!(
                    "git add '{}' failed: {}",
                    file, stderr
                )));
            }
        }

        // Commit
        let (stdout, stderr, exit_code) =
            Self::run_git(&["commit", "-m", message], &ctx.workspace)?;

        if exit_code != 0 {
            return Err(ToolError::execution_failed(&format!(
                "git commit failed: {}",
                stderr
            )));
        }

        Ok(ToolOutput::ok(serde_json::json!({
            "message": message,
            "files": files,
            "output": stdout.trim(),
        })))
    }

    fn execute_create_pr(
        title: &str,
        body: &str,
        ctx: &ExecutionContext,
    ) -> Result<ToolOutput, ToolError> {
        ctx.check_policy("github.write", None, Some("gh pr create"))?;

        let (stdout, stderr, exit_code) = Self::run_gh(
            &["pr", "create", "--title", title, "--body", body],
            &ctx.workspace,
        )?;

        if exit_code != 0 {
            return Err(ToolError::execution_failed(&format!(
                "gh pr create failed: {}",
                stderr
            )));
        }

        Ok(ToolOutput::ok(serde_json::json!({
            "title": title,
            "url": stdout.trim(),
        })))
    }
}

impl Default for GitHubTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for GitHubTool {
    fn name(&self) -> &str {
        "github"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            tool_id: "github-v1".to_string(),
            name: "github".to_string(),
            description: "Interact with GitHub: issues, branches, commits, PRs".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["list_issues", "create_branch", "commit", "create_pr"]
                    },
                    "branch": {"type": "string"},
                    "message": {"type": "string"},
                    "title": {"type": "string"},
                    "body": {"type": "string"},
                    "files": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["operation"]
            }),
            output_schema: serde_json::json!({"type": "object"}),
            permissions: vec!["github.read".to_string(), "github.write".to_string()],
            timeout_ms: 60_000,
            idempotent: false,
        }
    }

    fn execute(&self, input: &ToolInput, ctx: &ExecutionContext) -> Result<ToolOutput, ToolError> {
        let operation = input
            .params
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::invalid_input("Missing 'operation' parameter"))?;

        match operation {
            "list_issues" => Self::execute_list_issues(ctx),
            "create_branch" => {
                let branch = input
                    .params
                    .get("branch")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::invalid_input("Missing 'branch' parameter"))?;
                Self::execute_create_branch(branch, ctx)
            }
            "commit" => {
                let message = input
                    .params
                    .get("message")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::invalid_input("Missing 'message' parameter"))?;
                let files: Vec<&str> = input
                    .params
                    .get("files")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();
                Self::execute_commit(message, &files, ctx)
            }
            "create_pr" => {
                let title = input
                    .params
                    .get("title")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::invalid_input("Missing 'title' parameter"))?;
                let body = input
                    .params
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Self::execute_create_pr(title, body, ctx)
            }
            _ => Err(ToolError::invalid_input(&format!(
                "Unknown operation: '{}'. Expected: list_issues, create_branch, commit, create_pr",
                operation
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_engine::PolicyEngine;
    use std::sync::Arc;
    use std::time::Duration;

    fn test_ctx() -> ExecutionContext {
        ExecutionContext {
            task_id: "t-1".to_string(),
            step_id: "s-1".to_string(),
            trace_id: "tr-1".to_string(),
            agent: "tester".to_string(),
            timeout: Duration::from_secs(10),
            policy: Arc::new(PolicyEngine::permissive()),
            workspace: ".".to_string(),
        }
    }

    fn deny_ctx() -> ExecutionContext {
        ExecutionContext {
            task_id: "t-1".to_string(),
            step_id: "s-1".to_string(),
            trace_id: "tr-1".to_string(),
            agent: "tester".to_string(),
            timeout: Duration::from_secs(10),
            policy: Arc::new(PolicyEngine::deny_all()),
            workspace: ".".to_string(),
        }
    }

    fn make_input(operation: &str) -> ToolInput {
        let mut params = std::collections::HashMap::new();
        params.insert("operation".to_string(), serde_json::json!(operation));
        ToolInput {
            tool_name: "github".to_string(),
            params,
        }
    }

    #[test]
    fn test_github_tool_name_and_schema() {
        let tool = GitHubTool::new();
        assert_eq!(tool.name(), "github");
        let schema = tool.schema();
        assert_eq!(schema.tool_id, "github-v1");
        assert!(schema.permissions.contains(&"github.read".to_string()));
    }

    #[test]
    fn test_github_invalid_operation() {
        let tool = GitHubTool::new();
        let input = make_input("delete_repo");
        let result = tool.execute(&input, &test_ctx());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind,
            crate::tool_protocol::ToolErrorKind::InvalidInput
        );
    }

    #[test]
    fn test_github_missing_operation() {
        let tool = GitHubTool::new();
        let input = ToolInput {
            tool_name: "github".to_string(),
            params: std::collections::HashMap::new(),
        };
        let result = tool.execute(&input, &test_ctx());
        assert!(result.is_err());
    }

    #[test]
    fn test_github_create_branch_missing_param() {
        let tool = GitHubTool::new();
        let input = make_input("create_branch");
        let result = tool.execute(&input, &test_ctx());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind,
            crate::tool_protocol::ToolErrorKind::InvalidInput
        );
    }

    #[test]
    fn test_github_commit_missing_message() {
        let tool = GitHubTool::new();
        let input = make_input("commit");
        let result = tool.execute(&input, &test_ctx());
        assert!(result.is_err());
    }

    #[test]
    fn test_github_create_pr_missing_title() {
        let tool = GitHubTool::new();
        let input = make_input("create_pr");
        let result = tool.execute(&input, &test_ctx());
        assert!(result.is_err());
    }

    #[test]
    fn test_github_policy_denied_list_issues() {
        let tool = GitHubTool::new();
        let input = make_input("list_issues");
        let result = tool.execute(&input, &deny_ctx());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind,
            crate::tool_protocol::ToolErrorKind::PolicyDenied
        );
    }

    #[test]
    fn test_github_policy_denied_create_branch() {
        let tool = GitHubTool::new();
        let mut params = std::collections::HashMap::new();
        params.insert("operation".to_string(), serde_json::json!("create_branch"));
        params.insert("branch".to_string(), serde_json::json!("feat/test"));
        let input = ToolInput { tool_name: "github".to_string(), params };
        let result = tool.execute(&input, &deny_ctx());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind,
            crate::tool_protocol::ToolErrorKind::PolicyDenied
        );
    }
}
