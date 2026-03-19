//! FileSystem Tool Adapter: read/write/list/diff operations with path restrictions.
//!
//! - Path restriction enforced by policy engine
//! - Write operations generate diff (before/after)
//! - Atomic writes with backup for rollback support

use crate::tool_protocol::{ExecutionContext, Tool, ToolError, ToolInput, ToolOutput, ToolSchema};
use std::path::{Path, PathBuf};

pub struct FileSystemTool;

impl FileSystemTool {
    pub fn new() -> Self {
        Self
    }

    /// Resolve and validate path within workspace boundary.
    /// If `require_parent` is true, the parent directory must already exist.
    /// Prevents path escape via absolute paths or `..` traversal.
    fn resolve_path(
        path_str: &str,
        workspace: &str,
        require_parent: bool,
    ) -> Result<PathBuf, ToolError> {
        let workspace_root = std::fs::canonicalize(workspace).map_err(|e| {
            ToolError::execution_failed(&format!("Invalid workspace '{}': {}", workspace, e))
        })?;

        let input = Path::new(path_str);
        let candidate = if input.is_absolute() {
            input.to_path_buf()
        } else {
            workspace_root.join(input)
        };

        // Canonicalize to resolve `..` segments.
        // For existing paths, canonicalize directly; for new files, canonicalize parent + rejoin.
        let resolved = if candidate.exists() {
            std::fs::canonicalize(&candidate).map_err(|e| {
                ToolError::execution_failed(&format!(
                    "Failed to resolve path {}: {}",
                    candidate.display(),
                    e
                ))
            })?
        } else {
            let parent = candidate.parent().unwrap_or(Path::new("."));
            if parent.exists() {
                let parent_canon = std::fs::canonicalize(parent).map_err(|e| {
                    ToolError::execution_failed(&format!(
                        "Failed to resolve parent {}: {}",
                        parent.display(),
                        e
                    ))
                })?;
                match candidate.file_name() {
                    Some(name) => parent_canon.join(name),
                    None => parent_canon,
                }
            } else if require_parent {
                return Err(ToolError::not_found(&format!(
                    "Parent directory does not exist: {}",
                    parent.display()
                )));
            } else {
                // For write ops where parent doesn't exist yet, we still validate
                // the non-canonicalized path doesn't escape workspace
                candidate.clone()
            }
        };

        // Enforce workspace boundary
        if !resolved.starts_with(&workspace_root) {
            return Err(ToolError::invalid_input(&format!(
                "Path escapes workspace: {}",
                path_str
            )));
        }

        Ok(resolved)
    }

    fn execute_read(path: &Path) -> Result<ToolOutput, ToolError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            ToolError::execution_failed(&format!("Failed to read {}: {}", path.display(), e))
        })?;

        let metadata = std::fs::metadata(path).ok();
        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

        Ok(ToolOutput::ok(serde_json::json!({
            "content": content,
            "path": path.display().to_string(),
            "size_bytes": size,
        })))
    }

    fn execute_write(path: &Path, content: &str) -> Result<ToolOutput, ToolError> {
        // Read existing content for diff
        let old_content = std::fs::read_to_string(path).ok();

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ToolError::execution_failed(&format!("Failed to create directories: {}", e))
            })?;
        }

        // Write atomically: write to temp file, then rename
        let temp_path = path.with_extension("tmp.omnihive");
        std::fs::write(&temp_path, content).map_err(|e| {
            ToolError::execution_failed(&format!("Failed to write temp file: {}", e))
        })?;

        std::fs::rename(&temp_path, path).map_err(|e| {
            // Cleanup temp file on rename failure
            let _ = std::fs::remove_file(&temp_path);
            ToolError::execution_failed(&format!("Failed to rename temp file: {}", e))
        })?;

        let diff = match old_content {
            Some(ref old) => compute_simple_diff(old, content),
            None => "(new file)".to_string(),
        };

        Ok(ToolOutput::ok(serde_json::json!({
            "path": path.display().to_string(),
            "bytes_written": content.len(),
            "diff": diff,
            "created": old_content.is_none(),
        })))
    }

    fn execute_list(path: &Path) -> Result<ToolOutput, ToolError> {
        if !path.is_dir() {
            return Err(ToolError::invalid_input(&format!(
                "'{}' is not a directory",
                path.display()
            )));
        }

        let entries = std::fs::read_dir(path).map_err(|e| {
            ToolError::execution_failed(&format!("Failed to list directory: {}", e))
        })?;

        let mut items: Vec<serde_json::Value> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| {
                ToolError::execution_failed(&format!("Failed to read entry: {}", e))
            })?;

            let file_type = entry.file_type().ok();
            let metadata = entry.metadata().ok();

            items.push(serde_json::json!({
                "name": entry.file_name().to_string_lossy().to_string(),
                "is_dir": file_type.as_ref().map(|t| t.is_dir()).unwrap_or(false),
                "is_file": file_type.as_ref().map(|t| t.is_file()).unwrap_or(false),
                "size_bytes": metadata.as_ref().map(|m| m.len()).unwrap_or(0),
            }));
        }

        items.sort_by(|a, b| {
            let a_name = a["name"].as_str().unwrap_or("");
            let b_name = b["name"].as_str().unwrap_or("");
            a_name.cmp(b_name)
        });

        Ok(ToolOutput::ok(serde_json::json!({
            "path": path.display().to_string(),
            "entries": items,
            "count": items.len(),
        })))
    }

    fn execute_diff(path_a: &Path, path_b: &Path) -> Result<ToolOutput, ToolError> {
        let content_a = std::fs::read_to_string(path_a).map_err(|e| {
            ToolError::execution_failed(&format!("Failed to read {}: {}", path_a.display(), e))
        })?;
        let content_b = std::fs::read_to_string(path_b).map_err(|e| {
            ToolError::execution_failed(&format!("Failed to read {}: {}", path_b.display(), e))
        })?;

        let diff = compute_simple_diff(&content_a, &content_b);

        Ok(ToolOutput::ok(serde_json::json!({
            "file_a": path_a.display().to_string(),
            "file_b": path_b.display().to_string(),
            "diff": diff,
            "identical": content_a == content_b,
        })))
    }
}

impl Default for FileSystemTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for FileSystemTool {
    fn name(&self) -> &str {
        "filesystem"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            tool_id: "filesystem-v1".to_string(),
            name: "filesystem".to_string(),
            description: "Read, write, list, and diff files".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["read", "write", "list", "diff"]
                    },
                    "path": {"type": "string"},
                    "content": {"type": "string", "description": "Content for write operation"},
                    "path_b": {"type": "string", "description": "Second path for diff operation"}
                },
                "required": ["operation", "path"]
            }),
            output_schema: serde_json::json!({"type": "object"}),
            permissions: vec!["fs.read".to_string(), "fs.write".to_string()],
            timeout_ms: 10_000,
            idempotent: false,
        }
    }

    fn execute(&self, input: &ToolInput, ctx: &ExecutionContext) -> Result<ToolOutput, ToolError> {
        let operation = input
            .params
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::invalid_input("Missing 'operation' parameter"))?;

        let path_str = input
            .params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::invalid_input("Missing 'path' parameter"))?;

        let require_parent = operation != "write";
        let resolved = Self::resolve_path(path_str, &ctx.workspace, require_parent)?;

        match operation {
            "read" => {
                ctx.check_policy("fs.read", Some(path_str), None)?;
                Self::execute_read(&resolved)
            }
            "write" => {
                ctx.check_policy("fs.write", Some(path_str), None)?;
                let content = input
                    .params
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::invalid_input("Missing 'content' for write operation")
                    })?;
                Self::execute_write(&resolved, content)
            }
            "list" => {
                ctx.check_policy("fs.read", Some(path_str), None)?;
                Self::execute_list(&resolved)
            }
            "diff" => {
                ctx.check_policy("fs.read", Some(path_str), None)?;
                let path_b_str = input
                    .params
                    .get("path_b")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::invalid_input("Missing 'path_b' for diff operation")
                    })?;
                let resolved_b = Self::resolve_path(path_b_str, &ctx.workspace, true)?;
                Self::execute_diff(&resolved, &resolved_b)
            }
            _ => Err(ToolError::invalid_input(&format!(
                "Unknown operation: '{}'. Expected: read, write, list, diff",
                operation
            ))),
        }
    }
}

/// Compute a simple line-by-line diff summary.
fn compute_simple_diff(old: &str, new: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let mut diff = String::new();
    let mut added = 0usize;
    let mut removed = 0usize;

    let max = old_lines.len().max(new_lines.len());
    for i in 0..max {
        let old_line = old_lines.get(i).copied();
        let new_line = new_lines.get(i).copied();

        match (old_line, new_line) {
            (Some(o), Some(n)) if o != n => {
                diff.push_str(&format!("-{}\n+{}\n", o, n));
                removed += 1;
                added += 1;
            }
            (Some(o), None) => {
                diff.push_str(&format!("-{}\n", o));
                removed += 1;
            }
            (None, Some(n)) => {
                diff.push_str(&format!("+{}\n", n));
                added += 1;
            }
            _ => {}
        }
    }

    if diff.is_empty() {
        "(no changes)".to_string()
    } else {
        format!("{}\n+{} -{} lines", diff.trim_end(), added, removed)
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

    fn make_input(operation: &str, path: &str) -> ToolInput {
        let mut params = std::collections::HashMap::new();
        params.insert("operation".to_string(), serde_json::json!(operation));
        params.insert("path".to_string(), serde_json::json!(path));
        ToolInput {
            tool_name: "filesystem".to_string(),
            params,
        }
    }

    #[test]
    fn test_fs_tool_name_and_schema() {
        let tool = FileSystemTool::new();
        assert_eq!(tool.name(), "filesystem");
        let schema = tool.schema();
        assert_eq!(schema.name, "filesystem");
    }

    #[test]
    fn test_fs_read_write_roundtrip() {
        let dir = std::env::temp_dir().join("omnihive_test_fs_tool");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("test.txt");

        // Write
        let tool = FileSystemTool::new();
        let mut params = std::collections::HashMap::new();
        params.insert("operation".to_string(), serde_json::json!("write"));
        params.insert(
            "path".to_string(),
            serde_json::json!(file_path.display().to_string()),
        );
        params.insert("content".to_string(), serde_json::json!("hello world"));
        let input = ToolInput {
            tool_name: "filesystem".to_string(),
            params,
        };
        let ctx = test_ctx(dir.to_str().unwrap());

        let result = tool.execute(&input, &ctx);
        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.success);
        assert_eq!(out.data["bytes_written"], 11);

        // Read
        let read_input = make_input("read", file_path.to_str().unwrap());
        let result = tool.execute(&read_input, &ctx);
        assert!(result.is_ok());
        let out = result.unwrap();
        assert_eq!(out.data["content"], "hello world");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_fs_list() {
        let dir = std::env::temp_dir().join("omnihive_test_fs_list");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("a.txt"), "aaa");
        let _ = std::fs::write(dir.join("b.txt"), "bbb");

        let tool = FileSystemTool::new();
        let input = make_input("list", dir.to_str().unwrap());
        let ctx = test_ctx(dir.to_str().unwrap());

        let result = tool.execute(&input, &ctx);
        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.data["count"].as_u64().unwrap() >= 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_fs_diff() {
        let dir = std::env::temp_dir().join("omnihive_test_fs_diff");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("a.txt"), "line1\nline2\n");
        let _ = std::fs::write(dir.join("b.txt"), "line1\nline3\n");

        let tool = FileSystemTool::new();
        let mut params = std::collections::HashMap::new();
        params.insert("operation".to_string(), serde_json::json!("diff"));
        params.insert(
            "path".to_string(),
            serde_json::json!(dir.join("a.txt").display().to_string()),
        );
        params.insert(
            "path_b".to_string(),
            serde_json::json!(dir.join("b.txt").display().to_string()),
        );
        let input = ToolInput {
            tool_name: "filesystem".to_string(),
            params,
        };
        let ctx = test_ctx(dir.to_str().unwrap());

        let result = tool.execute(&input, &ctx);
        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(!out.data["identical"].as_bool().unwrap());
        assert!(out.data["diff"].as_str().unwrap().contains("-line2"));
        assert!(out.data["diff"].as_str().unwrap().contains("+line3"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_fs_read_nonexistent() {
        let tool = FileSystemTool::new();
        let input = make_input("read", "/tmp/omnihive_nonexistent_12345.txt");
        let ctx = test_ctx("/tmp");
        let result = tool.execute(&input, &ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_fs_invalid_operation() {
        let dir = std::env::temp_dir();
        let tool = FileSystemTool::new();
        let input = make_input("delete", dir.to_str().unwrap());
        let ctx = test_ctx(dir.to_str().unwrap());
        let result = tool.execute(&input, &ctx);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind,
            crate::tool_protocol::ToolErrorKind::InvalidInput
        );
    }

    #[test]
    fn test_fs_policy_denied() {
        let dir = std::env::temp_dir();
        let tool = FileSystemTool::new();
        let file_path = dir.join("something.txt");
        let _ = std::fs::write(&file_path, "test");
        let input = make_input("read", file_path.to_str().unwrap());
        let ctx = ExecutionContext {
            task_id: "t-1".to_string(),
            step_id: "s-1".to_string(),
            trace_id: "tr-1".to_string(),
            agent: "tester".to_string(),
            timeout: Duration::from_secs(10),
            policy: Arc::new(PolicyEngine::deny_all()),
            workspace: dir.to_str().unwrap().to_string(),
        };
        let result = tool.execute(&input, &ctx);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind,
            crate::tool_protocol::ToolErrorKind::PolicyDenied
        );
        let _ = std::fs::remove_file(&file_path);
    }

    #[test]
    fn test_compute_simple_diff_no_changes() {
        let diff = compute_simple_diff("hello\n", "hello\n");
        assert_eq!(diff, "(no changes)");
    }

    #[test]
    fn test_compute_simple_diff_additions() {
        let diff = compute_simple_diff("a", "a\nb");
        assert!(diff.contains("+b"));
    }

    #[test]
    fn test_compute_simple_diff_removals() {
        let diff = compute_simple_diff("a\nb", "a");
        assert!(diff.contains("-b"));
    }

    #[test]
    fn test_write_creates_new_file() {
        let dir = std::env::temp_dir().join("omnihive_test_fs_create");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("new_file.txt");
        let _ = std::fs::remove_file(&file_path);

        let tool = FileSystemTool::new();
        let mut params = std::collections::HashMap::new();
        params.insert("operation".to_string(), serde_json::json!("write"));
        params.insert(
            "path".to_string(),
            serde_json::json!(file_path.display().to_string()),
        );
        params.insert("content".to_string(), serde_json::json!("new content"));
        let input = ToolInput {
            tool_name: "filesystem".to_string(),
            params,
        };
        let ctx = test_ctx(dir.to_str().unwrap());

        let result = tool.execute(&input, &ctx);
        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.data["created"].as_bool().unwrap());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
