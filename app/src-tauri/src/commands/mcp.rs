use crate::models::*;
use tauri::command;

/// List all configured MCP servers from settings.
#[command]
pub fn list_mcp_servers() -> Result<Vec<McpServerConfig>, String> {
    let settings = crate::commands::settings::load_settings()?;
    Ok(settings.mcp_servers)
}

/// Add a new MCP server configuration.
#[command]
pub fn add_mcp_server(server: McpServerConfig) -> Result<AppSettings, String> {
    let mut settings = crate::commands::settings::load_settings()?;

    // Check for duplicate ID
    if settings.mcp_servers.iter().any(|s| s.id == server.id) {
        return Err(format!("MCP server with ID '{}' already exists", server.id));
    }

    settings.mcp_servers.push(server);
    crate::commands::settings::save_settings(settings.clone())?;
    Ok(settings)
}

/// Update an existing MCP server configuration.
#[command]
pub fn update_mcp_server(server: McpServerConfig) -> Result<AppSettings, String> {
    let mut settings = crate::commands::settings::load_settings()?;

    if let Some(existing) = settings.mcp_servers.iter_mut().find(|s| s.id == server.id) {
        *existing = server;
    } else {
        return Err(format!("MCP server '{}' not found", server.id));
    }

    crate::commands::settings::save_settings(settings.clone())?;
    Ok(settings)
}

/// Remove an MCP server configuration.
#[command]
pub fn remove_mcp_server(server_id: String) -> Result<AppSettings, String> {
    let mut settings = crate::commands::settings::load_settings()?;
    settings.mcp_servers.retain(|s| s.id != server_id);
    crate::commands::settings::save_settings(settings.clone())?;
    Ok(settings)
}

/// Get a list of well-known MCP servers that users can quickly add.
#[command]
pub fn get_mcp_presets() -> Result<Vec<McpPreset>, String> {
    Ok(vec![
        McpPreset {
            id: "web-search".to_string(),
            name: "Web Search".to_string(),
            description: "Search the web using Tavily, Brave, or SerpAPI".to_string(),
            server_type: "stdio".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@anthropic/mcp-server-web-search".to_string(),
            ],
            env_keys: vec!["TAVILY_API_KEY".to_string()],
            category: "search".to_string(),
        },
        McpPreset {
            id: "filesystem".to_string(),
            name: "Filesystem".to_string(),
            description: "Read, write, and manage files on the local filesystem".to_string(),
            server_type: "stdio".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@anthropic/mcp-server-filesystem".to_string(),
            ],
            env_keys: vec![],
            category: "tools".to_string(),
        },
        McpPreset {
            id: "github".to_string(),
            name: "GitHub".to_string(),
            description: "Manage GitHub repos, issues, PRs, and more".to_string(),
            server_type: "stdio".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@anthropic/mcp-server-github".to_string()],
            env_keys: vec!["GITHUB_TOKEN".to_string()],
            category: "dev".to_string(),
        },
        McpPreset {
            id: "slack".to_string(),
            name: "Slack".to_string(),
            description: "Send messages and manage Slack workspaces".to_string(),
            server_type: "stdio".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@anthropic/mcp-server-slack".to_string()],
            env_keys: vec!["SLACK_BOT_TOKEN".to_string()],
            category: "communication".to_string(),
        },
        McpPreset {
            id: "postgres".to_string(),
            name: "PostgreSQL".to_string(),
            description: "Query and manage PostgreSQL databases".to_string(),
            server_type: "stdio".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@anthropic/mcp-server-postgres".to_string(),
            ],
            env_keys: vec!["DATABASE_URL".to_string()],
            category: "database".to_string(),
        },
        McpPreset {
            id: "brave-search".to_string(),
            name: "Brave Search".to_string(),
            description: "Search the web using the Brave Search API".to_string(),
            server_type: "stdio".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@anthropic/mcp-server-brave-search".to_string(),
            ],
            env_keys: vec!["BRAVE_API_KEY".to_string()],
            category: "search".to_string(),
        },
        McpPreset {
            id: "memory".to_string(),
            name: "Memory".to_string(),
            description: "Persistent knowledge graph memory for agents".to_string(),
            server_type: "stdio".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@anthropic/mcp-server-memory".to_string()],
            env_keys: vec![],
            category: "tools".to_string(),
        },
        McpPreset {
            id: "puppeteer".to_string(),
            name: "Puppeteer".to_string(),
            description: "Browser automation - navigate, screenshot, interact with web pages"
                .to_string(),
            server_type: "stdio".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@anthropic/mcp-server-puppeteer".to_string(),
            ],
            env_keys: vec![],
            category: "tools".to_string(),
        },
    ])
}

/// MCP preset for quick-add UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub server_type: String,
    pub command: String,
    pub args: Vec<String>,
    pub env_keys: Vec<String>,
    pub category: String,
}
