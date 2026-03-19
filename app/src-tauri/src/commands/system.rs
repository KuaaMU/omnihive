use crate::commands::runtime::{find_binary, resolve_engine_binary, silent_command};
use crate::models::*;
use tauri::command;

#[command]
pub fn detect_system() -> Result<SystemInfo, String> {
    let os = detect_os();
    let arch = detect_arch();
    let shells = detect_shells();
    let default_shell = shells
        .iter()
        .find(|s| s.available)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let tools = detect_tools();
    let node_version = get_version("node", &["--version"]);
    let npm_version = get_version("npm", &["--version"]);

    Ok(SystemInfo {
        os,
        arch,
        default_shell,
        shells,
        tools,
        node_version,
        npm_version,
    })
}

#[command]
pub fn install_tool(tool_name: String, install_dir: Option<String>) -> Result<String, String> {
    // Verify npm is available
    if find_binary("npm").is_none() {
        return Err(
            "npm is not installed. Please install Node.js first from https://nodejs.org/"
                .to_string(),
        );
    }

    let (package, needs_npm) = match tool_name.as_str() {
        "claude" => ("@anthropic-ai/claude-code", true),
        "codex" => ("@openai/codex", true),
        "opencode" => ("opencode", false),
        _ => return Err(format!("Unknown tool: {}", tool_name)),
    };

    if !needs_npm {
        return Err(format!(
            "{} cannot be installed via npm. Please install it manually.",
            tool_name
        ));
    }

    let mut args = vec!["install", "-g", package];

    // If a custom prefix is specified, use it
    let prefix_flag;
    if let Some(ref dir) = install_dir {
        prefix_flag = format!("--prefix={}", dir);
        args.push(&prefix_flag);
    }

    let npm_path = find_binary("npm").ok_or_else(|| "npm not found in PATH".to_string())?;

    // On Windows, npm might be a .cmd file
    #[cfg(target_os = "windows")]
    let output = if npm_path.ends_with(".cmd") || npm_path.ends_with(".bat") {
        silent_command("cmd")
            .arg("/C")
            .arg(&npm_path)
            .args(&args)
            .output()
    } else {
        silent_command(&npm_path).args(&args).output()
    };

    #[cfg(not(target_os = "windows"))]
    let output = silent_command(&npm_path).args(&args).output();

    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).to_string();

            if o.status.success() {
                Ok(format!("Successfully installed {}.\n{}", package, stdout))
            } else {
                Err(format!(
                    "Installation failed (exit {}):\n{}\n{}",
                    o.status, stdout, stderr
                ))
            }
        }
        Err(e) => Err(format!("Failed to run npm: {}", e)),
    }
}

#[command]
pub fn check_engine(engine: String) -> Result<String, String> {
    resolve_engine_binary(&engine)
}

// ===== Detection helpers =====

fn detect_os() -> String {
    #[cfg(target_os = "windows")]
    {
        "windows".to_string()
    }
    #[cfg(target_os = "macos")]
    {
        "macos".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "linux".to_string()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        std::env::consts::OS.to_string()
    }
}

fn detect_arch() -> String {
    std::env::consts::ARCH.to_string()
}

fn detect_shells() -> Vec<ShellInfo> {
    let mut shells = vec![
        // PowerShell (Windows primary, also available on macOS/Linux)
        detect_shell_info(
            "powershell",
            &["powershell", "pwsh"],
            &["-Command", "$PSVersionTable.PSVersion.ToString()"],
        ),
        // Bash
        detect_shell_info("bash", &["bash"], &["--version"]),
    ];

    // Cmd (Windows only)
    #[cfg(target_os = "windows")]
    shells.push(ShellInfo {
        name: "cmd".to_string(),
        path: find_binary("cmd"),
        version: None,
        available: find_binary("cmd").is_some(),
    });

    // Zsh (macOS/Linux)
    #[cfg(not(target_os = "windows"))]
    shells.push(detect_shell_info("zsh", &["zsh"], &["--version"]));

    shells
}

fn detect_shell_info(name: &str, binaries: &[&str], version_args: &[&str]) -> ShellInfo {
    for bin in binaries {
        if let Some(path) = find_binary(bin) {
            let version = get_version(bin, version_args);
            return ShellInfo {
                name: name.to_string(),
                path: Some(path),
                version,
                available: true,
            };
        }
    }

    ShellInfo {
        name: name.to_string(),
        path: None,
        version: None,
        available: false,
    }
}

fn detect_tools() -> Vec<ToolInfo> {
    vec![
        detect_tool_info(
            "claude",
            "Claude Code",
            &["claude"],
            &["--version"],
            "npm install -g @anthropic-ai/claude-code",
            "https://docs.anthropic.com/en/docs/claude-code",
        ),
        detect_tool_info(
            "codex",
            "Codex CLI",
            &["codex"],
            &["--version"],
            "npm install -g @openai/codex",
            "https://github.com/openai/codex",
        ),
        detect_tool_info(
            "opencode",
            "OpenCode",
            &["opencode"],
            &["--version"],
            "go install github.com/opencode-ai/opencode@latest",
            "https://github.com/opencode-ai/opencode",
        ),
    ]
}

fn detect_tool_info(
    name: &str,
    display_name: &str,
    binaries: &[&str],
    version_args: &[&str],
    install_command: &str,
    install_url: &str,
) -> ToolInfo {
    for bin in binaries {
        if let Some(path) = find_binary(bin) {
            let version = get_version(bin, version_args);
            return ToolInfo {
                name: name.to_string(),
                display_name: display_name.to_string(),
                available: true,
                version,
                path: Some(path),
                install_command: install_command.to_string(),
                install_url: install_url.to_string(),
            };
        }
    }

    ToolInfo {
        name: name.to_string(),
        display_name: display_name.to_string(),
        available: false,
        version: None,
        path: None,
        install_command: install_command.to_string(),
        install_url: install_url.to_string(),
    }
}

fn get_version(cmd: &str, args: &[&str]) -> Option<String> {
    // Resolve the full path first
    let full_path = find_binary(cmd)?;

    #[cfg(target_os = "windows")]
    let output = if full_path.ends_with(".cmd") || full_path.ends_with(".bat") {
        silent_command("cmd")
            .arg("/C")
            .arg(&full_path)
            .args(args)
            .output()
    } else {
        silent_command(&full_path).args(args).output()
    };

    #[cfg(not(target_os = "windows"))]
    let output = silent_command(&full_path).args(args).output();

    output.ok().filter(|o| o.status.success()).map(|o| {
        let out = String::from_utf8_lossy(&o.stdout);
        // Take just the first line and trim version prefixes
        let line = out.lines().next().unwrap_or("").trim();
        // Strip common prefixes like "v" from version strings
        if let Some(stripped) = line.strip_prefix('v') {
            stripped.to_string()
        } else {
            line.to_string()
        }
    })
}
