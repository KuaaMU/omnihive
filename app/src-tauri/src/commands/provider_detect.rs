use crate::models::*;
use std::path::PathBuf;
use tauri::command;

/// Mask an API key, showing only the first 8 and last 4 characters.
fn mask_key(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.len() <= 12 {
        return "*".repeat(trimmed.len());
    }
    format!("{}...{}", &trimmed[..8], &trimmed[trimmed.len() - 4..])
}

/// Scan environment variables and config files for existing API provider configurations.
/// Returns detected providers along with debug info about paths checked.
#[command]
pub fn detect_providers() -> Result<Vec<DetectedProvider>, String> {
    let mut providers = Vec::new();

    // 1. Environment variables (most reliable on Windows GUI apps if set system-wide)
    detect_env_providers(&mut providers);

    // 2. Claude Code config
    detect_claude_config(&mut providers);

    // 3. Codex CLI config
    detect_codex_config(&mut providers);

    // 4. OpenCode config
    detect_opencode_config(&mut providers);

    // 5. Cursor config
    detect_cursor_config(&mut providers);

    // Deduplicate by api_key (keep the first occurrence)
    let mut seen_keys = std::collections::HashSet::new();
    providers.retain(|p| seen_keys.insert(p.api_key.clone()));

    Ok(providers)
}

/// Export selected providers as JSON (with masked keys for display).
#[command]
pub fn export_providers(provider_ids: Vec<String>) -> Result<String, String> {
    let settings = crate::commands::settings::load_settings()?;
    let selected: Vec<_> = settings
        .providers
        .iter()
        .filter(|p| provider_ids.contains(&p.id))
        .cloned()
        .collect();

    // Mask API keys for export
    let masked: Vec<_> = selected
        .iter()
        .map(|p| {
            let mut cloned = p.clone();
            cloned.api_key = mask_key(&cloned.api_key);
            cloned
        })
        .collect();

    serde_json::to_string_pretty(&masked)
        .map_err(|e| format!("Failed to serialize providers: {}", e))
}

/// Import providers from a JSON string, adding them to current settings.
#[command]
pub fn import_providers(json: String) -> Result<AppSettings, String> {
    let imported: Vec<AiProvider> =
        serde_json::from_str(&json).map_err(|e| format!("Failed to parse provider JSON: {}", e))?;

    let mut settings = crate::commands::settings::load_settings()?;

    for provider in imported {
        // Skip if API key looks masked
        if provider.api_key.contains("...") || provider.api_key.chars().all(|c| c == '*') {
            continue;
        }
        // Skip duplicates by ID
        if settings.providers.iter().any(|p| p.id == provider.id) {
            continue;
        }
        settings.providers.push(provider);
    }

    crate::commands::settings::save_settings(settings.clone())?;
    Ok(settings)
}

// ===== Detection helpers =====

fn detect_env_providers(providers: &mut Vec<DetectedProvider>) {
    let env_configs: &[(&str, &str, &str, &str, &str)] = &[
        (
            "ANTHROPIC_API_KEY",
            "anthropic",
            "Anthropic (Claude)",
            "https://api.anthropic.com",
            "claude-sonnet-4-20250514",
        ),
        (
            "CLAUDE_API_KEY",
            "anthropic",
            "Anthropic (Claude)",
            "https://api.anthropic.com",
            "claude-sonnet-4-20250514",
        ),
        (
            "OPENAI_API_KEY",
            "openai",
            "OpenAI",
            "https://api.openai.com/v1",
            "gpt-4o",
        ),
        (
            "OPENROUTER_API_KEY",
            "openrouter",
            "OpenRouter",
            "https://openrouter.ai/api/v1",
            "anthropic/claude-sonnet-4-20250514",
        ),
        (
            "GEMINI_API_KEY",
            "gemini",
            "Google Gemini",
            "https://generativelanguage.googleapis.com/v1beta",
            "gemini-2.5-pro",
        ),
        (
            "GOOGLE_API_KEY",
            "gemini",
            "Google Gemini",
            "https://generativelanguage.googleapis.com/v1beta",
            "gemini-2.5-pro",
        ),
        (
            "DEEPSEEK_API_KEY",
            "deepseek",
            "DeepSeek",
            "https://api.deepseek.com",
            "deepseek-chat",
        ),
        (
            "GROQ_API_KEY",
            "groq",
            "Groq",
            "https://api.groq.com/openai/v1",
            "llama-3.3-70b-versatile",
        ),
    ];

    for (env_var, provider_type, name, base_url, model) in env_configs {
        if let Ok(key) = std::env::var(env_var) {
            let trimmed = key.trim().to_string();
            if !trimmed.is_empty() && trimmed.len() > 10 {
                providers.push(DetectedProvider {
                    source: format!("ENV:{}", env_var),
                    provider_type: provider_type.to_string(),
                    api_key_preview: mask_key(&trimmed),
                    api_key: trimmed,
                    api_base_url: base_url.to_string(),
                    suggested_name: name.to_string(),
                    suggested_model: model.to_string(),
                });
            }
        }
    }
}

fn detect_claude_config(providers: &mut Vec<DetectedProvider>) {
    let paths = get_claude_config_paths();

    for path in paths {
        if !path.exists() {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let source = format!(
            "CC:{}",
            path.file_name().unwrap_or_default().to_string_lossy()
        );

        // Claude Code settings.json may have apiKey at top level
        for key_field in ["apiKey", "primaryApiKey", "api_key"] {
            if let Some(key) = json.get(key_field).and_then(|v| v.as_str()) {
                if !key.is_empty() && key.len() > 10 {
                    providers.push(DetectedProvider {
                        source: source.clone(),
                        provider_type: "anthropic".to_string(),
                        api_key_preview: mask_key(key),
                        api_key: key.to_string(),
                        api_base_url: "https://api.anthropic.com".to_string(),
                        suggested_name: "Claude Code".to_string(),
                        suggested_model: "claude-sonnet-4-20250514".to_string(),
                    });
                }
            }
        }

        // Check env overrides stored in Claude settings
        if let Some(env_obj) = json.get("env").and_then(|v| v.as_object()) {
            if let Some(key) = env_obj.get("ANTHROPIC_API_KEY").and_then(|v| v.as_str()) {
                if !key.is_empty() && key.len() > 10 {
                    providers.push(DetectedProvider {
                        source: source.clone(),
                        provider_type: "anthropic".to_string(),
                        api_key_preview: mask_key(key),
                        api_key: key.to_string(),
                        api_base_url: "https://api.anthropic.com".to_string(),
                        suggested_name: "Claude Code (env)".to_string(),
                        suggested_model: "claude-sonnet-4-20250514".to_string(),
                    });
                }
            }
        }

        // Check for provider configurations inside settings
        if let Some(providers_val) = json.get("providers").and_then(|v| v.as_array()) {
            for p in providers_val {
                let key = p
                    .get("apiKey")
                    .or_else(|| p.get("api_key"))
                    .or_else(|| p.get("key"))
                    .and_then(|v| v.as_str());
                let ptype = p
                    .get("type")
                    .or_else(|| p.get("provider_type"))
                    .or_else(|| p.get("provider"))
                    .and_then(|v| v.as_str());
                if let (Some(key), Some(ptype)) = (key, ptype) {
                    if !key.is_empty() && key.len() > 10 {
                        let base_url = p
                            .get("baseUrl")
                            .or_else(|| p.get("api_base_url"))
                            .or_else(|| p.get("baseURL"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("https://api.anthropic.com");
                        providers.push(DetectedProvider {
                            source: source.clone(),
                            provider_type: ptype.to_string(),
                            api_key_preview: mask_key(key),
                            api_key: key.to_string(),
                            api_base_url: base_url.to_string(),
                            suggested_name: format!("{} (Claude Code)", ptype),
                            suggested_model: "claude-sonnet-4-20250514".to_string(),
                        });
                    }
                }
            }
        }
    }
}

fn detect_codex_config(providers: &mut Vec<DetectedProvider>) {
    let paths = get_codex_config_paths();

    for path in paths {
        if !path.exists() {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let source = "CDX".to_string();

        for key_field in ["apiKey", "api_key", "key"] {
            if let Some(key) = json.get(key_field).and_then(|v| v.as_str()) {
                if !key.is_empty() && key.len() > 10 {
                    let model = json
                        .get("model")
                        .and_then(|v| v.as_str())
                        .unwrap_or("gpt-4o");
                    providers.push(DetectedProvider {
                        source: source.clone(),
                        provider_type: "openai".to_string(),
                        api_key_preview: mask_key(key),
                        api_key: key.to_string(),
                        api_base_url: "https://api.openai.com/v1".to_string(),
                        suggested_name: "Codex CLI".to_string(),
                        suggested_model: model.to_string(),
                    });
                }
            }
        }
    }
}

fn detect_opencode_config(providers: &mut Vec<DetectedProvider>) {
    let paths = get_opencode_config_paths();

    for path in paths {
        if !path.exists() {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Parse TOML with regex-like approach for key=value pairs
        let mut api_key = String::new();
        let mut provider_type = String::new();
        let mut model = String::new();

        for line in content.lines() {
            let trimmed = line.trim();
            // Skip comments
            if trimmed.starts_with('#') || trimmed.starts_with("//") {
                continue;
            }
            if let Some((k, v)) = parse_toml_kv(trimmed) {
                match k.as_str() {
                    "api_key" | "apiKey" | "key" => api_key = v,
                    "provider" | "provider_type" | "type" => provider_type = v,
                    "model" => model = v,
                    _ => {}
                }
            }
        }

        if !api_key.is_empty() && api_key.len() > 10 {
            if provider_type.is_empty() {
                provider_type = "openai".to_string();
            }
            if model.is_empty() {
                model = "gpt-4o".to_string();
            }
            providers.push(DetectedProvider {
                source: "OC".to_string(),
                provider_type,
                api_key_preview: mask_key(&api_key),
                api_key,
                api_base_url: "https://api.openai.com/v1".to_string(),
                suggested_name: "OpenCode".to_string(),
                suggested_model: model,
            });
        }
    }
}

fn detect_cursor_config(providers: &mut Vec<DetectedProvider>) {
    // Cursor stores config in %APPDATA%\Cursor\User\settings.json on Windows
    let mut paths = Vec::new();

    if let Some(appdata) = dirs::config_dir() {
        paths.push(appdata.join("Cursor").join("User").join("settings.json"));
    }
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".cursor").join("config.json"));
    }

    for path in paths {
        if !path.exists() {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Cursor might store API keys for custom providers
        for key_field in ["openai.apiKey", "anthropic.apiKey"] {
            let parts: Vec<&str> = key_field.split('.').collect();
            if parts.len() == 2 {
                if let Some(key) = json
                    .get(parts[0])
                    .and_then(|v| v.get(parts[1]))
                    .and_then(|v| v.as_str())
                {
                    if !key.is_empty() && key.len() > 10 {
                        let ptype = parts[0];
                        providers.push(DetectedProvider {
                            source: "Cursor".to_string(),
                            provider_type: ptype.to_string(),
                            api_key_preview: mask_key(key),
                            api_key: key.to_string(),
                            api_base_url: if ptype == "anthropic" {
                                "https://api.anthropic.com".to_string()
                            } else {
                                "https://api.openai.com/v1".to_string()
                            },
                            suggested_name: format!("Cursor ({})", ptype),
                            suggested_model: if ptype == "anthropic" {
                                "claude-sonnet-4-20250514".to_string()
                            } else {
                                "gpt-4o".to_string()
                            },
                        });
                    }
                }
            }
        }
    }
}

/// Parse a simple TOML key=value line, stripping quotes from the value.
fn parse_toml_kv(line: &str) -> Option<(String, String)> {
    let eq_idx = line.find('=')?;
    let key = line[..eq_idx].trim().to_string();
    let val = line[eq_idx + 1..].trim();
    let val = val.trim_matches('"').trim_matches('\'').to_string();
    if key.is_empty() || val.is_empty() {
        return None;
    }
    Some((key, val))
}

// ===== Config path resolution =====

fn get_claude_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // On Windows: ~/.claude/ is actually %USERPROFILE%\.claude\
    // dirs::home_dir() returns %USERPROFILE% on Windows
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".claude").join("settings.json"));
        paths.push(home.join(".claude.json"));
    }

    // Also check %APPDATA%\claude\ (some installations)
    if let Some(appdata) = dirs::config_dir() {
        paths.push(appdata.join("claude").join("settings.json"));
        paths.push(appdata.join("Claude").join("settings.json"));
    }

    // Also check %LOCALAPPDATA%\claude\ (rare but possible)
    if let Some(local) = dirs::data_local_dir() {
        paths.push(local.join("claude").join("settings.json"));
    }

    paths
}

fn get_codex_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".codex").join("config.json"));
        paths.push(home.join(".codex").join("settings.json"));
    }

    if let Some(config) = dirs::config_dir() {
        paths.push(config.join("codex").join("config.json"));
        paths.push(config.join("Codex").join("config.json"));
    }

    paths
}

fn get_opencode_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".opencode").join("config.toml"));
        paths.push(home.join(".opencode.toml"));
    }

    if let Some(config) = dirs::config_dir() {
        paths.push(config.join("opencode").join("config.toml"));
    }

    paths
}
