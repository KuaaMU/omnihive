use crate::engine::api_client;
use crate::models::*;
use std::path::PathBuf;
use tauri::command;

fn get_settings_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("omnihive")
        .join("settings.json")
}

fn default_settings() -> AppSettings {
    AppSettings {
        default_engine: "claude".to_string(),
        default_model: "sonnet".to_string(),
        max_daily_budget: 50.0,
        alert_at_budget: 30.0,
        loop_interval: 30,
        cycle_timeout: 1800,
        projects_dir: dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("omnihive")
            .join("projects")
            .display()
            .to_string(),
        providers: vec![],
        language: "en".to_string(),
        theme: "obsidian".to_string(),
        mcp_servers: vec![],
        skill_repos: vec![],
    }
}

#[command]
pub fn load_settings() -> Result<AppSettings, String> {
    let path = get_settings_path();
    if !path.exists() {
        let settings = default_settings();
        // Create parent dir and save defaults
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("Serialize error: {}", e))?;
        let _ = std::fs::write(&path, &json);
        return Ok(settings);
    }

    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read settings: {}", e))?;
    let settings: AppSettings =
        serde_json::from_str(&content).map_err(|e| format!("Parse error: {}", e))?;
    Ok(settings)
}

#[command]
pub fn save_settings(settings: AppSettings) -> Result<bool, String> {
    let path = get_settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create settings dir: {}", e))?;
    }
    let json =
        serde_json::to_string_pretty(&settings).map_err(|e| format!("Serialize error: {}", e))?;
    std::fs::write(&path, &json).map_err(|e| format!("Write error: {}", e))?;
    Ok(true)
}

// ===== Provider Management =====

#[command]
pub fn add_provider(provider: AiProvider) -> Result<AppSettings, String> {
    let mut settings = load_settings()?;

    // Check for duplicate
    if settings.providers.iter().any(|p| p.id == provider.id) {
        return Err(format!("Provider with id '{}' already exists", provider.id));
    }

    settings.providers.push(provider);
    save_settings(settings.clone())?;
    Ok(settings)
}

#[command]
pub fn update_provider(provider: AiProvider) -> Result<AppSettings, String> {
    let mut settings = load_settings()?;

    let idx = settings
        .providers
        .iter()
        .position(|p| p.id == provider.id)
        .ok_or_else(|| format!("Provider '{}' not found", provider.id))?;

    settings.providers[idx] = provider;
    save_settings(settings.clone())?;
    Ok(settings)
}

#[command]
pub fn remove_provider(provider_id: String) -> Result<AppSettings, String> {
    let mut settings = load_settings()?;
    settings.providers.retain(|p| p.id != provider_id);
    save_settings(settings.clone())?;
    Ok(settings)
}

/// Maps provider_type to (api_format, default_base_url).
pub fn derive_api_config(provider_type: &str) -> (&'static str, &'static str) {
    match provider_type {
        "anthropic" | "claude" => ("anthropic", "https://api.anthropic.com"),
        "openai" => ("openai", "https://api.openai.com/v1"),
        "openrouter" => ("openai", "https://openrouter.ai/api/v1"),
        "deepseek" => ("openai", "https://api.deepseek.com/v1"),
        "groq" => ("openai", "https://api.groq.com/openai/v1"),
        "mistral" => ("openai", "https://api.mistral.ai/v1"),
        "google" | "gemini" => (
            "openai",
            "https://generativelanguage.googleapis.com/v1beta/openai",
        ),
        _ => ("openai", ""),
    }
}

#[command]
pub fn test_provider(provider: AiProvider) -> Result<String, String> {
    // Basic field validation
    if provider.api_key.is_empty() {
        return Err("API key is required".to_string());
    }

    let (derived_format, derived_url) = derive_api_config(&provider.provider_type);

    let api_base_url = if provider.api_base_url.is_empty() {
        if derived_url.is_empty() {
            return Err("API base URL is required for custom providers".to_string());
        }
        derived_url.to_string()
    } else {
        provider.api_base_url.clone()
    };

    // Use provider's explicit api_format if set, otherwise derive from provider_type
    let api_format = if !provider.api_format.is_empty()
        && provider.api_format != "anthropic"
        && provider.provider_type != "anthropic"
        && provider.provider_type != "claude"
    {
        provider.api_format.clone()
    } else {
        derived_format.to_string()
    };

    let model = if provider.default_model.is_empty() {
        match provider.provider_type.as_str() {
            "anthropic" | "claude" => "claude-sonnet-4-20250514".to_string(),
            "openai" => "gpt-4o-mini".to_string(),
            "openrouter" => "anthropic/claude-sonnet-4-20250514".to_string(),
            "deepseek" => "deepseek-chat".to_string(),
            "groq" => "llama-3.1-8b-instant".to_string(),
            "mistral" => "mistral-small-latest".to_string(),
            _ => provider.default_model.clone(),
        }
    } else {
        provider.default_model.clone()
    };

    let config = api_client::ApiCallConfig {
        api_key: provider.api_key.clone(),
        api_base_url,
        model,
        system_prompt: "You are a connection test. Respond with exactly: OK".to_string(),
        user_message: "Say OK".to_string(),
        timeout_secs: 30,
        anthropic_version: if provider.anthropic_version.is_empty() {
            "2023-06-01".to_string()
        } else {
            provider.anthropic_version.clone()
        },
        extra_headers: provider.extra_headers.clone(),
        force_stream: provider.force_stream,
        api_format,
    };

    match api_client::call_api(&config) {
        Ok(resp) => Ok(format!(
            "Connection successful. Tokens: {} in / {} out. Response: {}",
            resp.input_tokens,
            resp.output_tokens,
            if resp.text.len() > 200 {
                format!("{}...", &resp.text[..200])
            } else {
                resp.text
            }
        )),
        Err(e) => Err(e),
    }
}
