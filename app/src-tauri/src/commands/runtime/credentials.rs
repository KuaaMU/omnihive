use crate::models::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// API credentials resolved at loop start.
pub(crate) struct ApiCredentials {
    pub engine_type: String,
    pub api_key: String,
    pub api_base_url: String,
    pub model: String,
    pub anthropic_version: String,
    pub extra_headers: HashMap<String, String>,
    pub force_stream: bool,
    pub api_format: String,
}

/// Result of auto-selecting the best available provider.
#[derive(serde::Serialize)]
pub struct SelectedProvider {
    pub provider_id: String,
    pub provider_name: String,
    pub provider_type: String,
    pub api_base_url: String,
    pub model: String,
    pub api_format: String,
}

// ===== App Settings Loading =====

pub(crate) fn load_app_settings() -> Result<AppSettings, String> {
    let path = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("omnihive")
        .join("settings.json");

    if !path.exists() {
        return Err("Settings file not found. Please configure settings first.".to_string());
    }

    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read settings: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse settings: {}", e))
}

// ===== Project Config Loading =====

pub(crate) fn load_project_config(dir: &Path) -> Result<FactoryConfig, String> {
    let config_path = dir.join("company.yaml");
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read company.yaml: {}", e))?;
    serde_yaml::from_str(&content).map_err(|e| format!("Failed to parse company.yaml: {}", e))
}

// ===== Model Name Resolution =====

pub(crate) fn resolve_model_name(engine_type: &str, model: &str) -> String {
    if model.contains('-') || model.contains('/') {
        return model.to_string();
    }
    match engine_type {
        "anthropic" => match model {
            "opus" => "claude-opus-4-20250514".to_string(),
            "sonnet" => "claude-sonnet-4-20250514".to_string(),
            "haiku" => "claude-3-5-haiku-20241022".to_string(),
            other => other.to_string(),
        },
        "openai" => match model {
            "opus" | "sonnet" => "gpt-4o".to_string(),
            "haiku" => "gpt-4o-mini".to_string(),
            other => other.to_string(),
        },
        _ => model.to_string(),
    }
}

// ===== API Credential Resolution =====

#[tracing::instrument(skip_all, fields(engine = %engine))]
pub(crate) fn resolve_api_credentials(engine: &str, model: &str) -> Result<ApiCredentials, String> {
    use crate::commands::settings::derive_api_config;

    // If engine is "auto" or empty, use auto-select
    if engine.is_empty() || engine == "auto" {
        let (mut creds, _) = auto_select_provider_internal()?;
        if !model.is_empty() && model != "auto" {
            creds.model = model.to_string();
        }
        return Ok(creds);
    }

    // 1. Try app-level settings (stored providers)
    if let Ok(settings) = load_app_settings() {
        let provider_type = match engine {
            "claude" => "anthropic",
            "openai" | "codex" => "openai",
            other => other,
        };

        let provider = settings
            .providers
            .iter()
            .find(|p| p.enabled && (p.provider_type == provider_type || p.provider_type == engine))
            .or_else(|| {
                settings
                    .providers
                    .iter()
                    .find(|p| p.enabled && p.engine == engine)
            });

        if let Some(provider) = provider {
            if !provider.api_key.is_empty() {
                let (derived_format, derived_url) = derive_api_config(&provider.provider_type);

                let api_base_url = if provider.api_base_url.is_empty() {
                    derived_url.to_string()
                } else {
                    provider.api_base_url.clone()
                };

                let resolved_model =
                    if !provider.default_model.is_empty() && provider.default_model.contains('-') {
                        provider.default_model.clone()
                    } else {
                        model.to_string()
                    };

                let api_format = if !provider.api_format.is_empty() {
                    provider.api_format.clone()
                } else {
                    derived_format.to_string()
                };

                return Ok(ApiCredentials {
                    engine_type: provider.provider_type.clone(),
                    api_key: provider.api_key.clone(),
                    api_base_url,
                    model: resolved_model,
                    anthropic_version: if provider.anthropic_version.is_empty() {
                        "2023-06-01".to_string()
                    } else {
                        provider.anthropic_version.clone()
                    },
                    extra_headers: provider.extra_headers.clone(),
                    force_stream: provider.force_stream,
                    api_format,
                });
            }
        }
    }

    // 2. Try environment variables
    let env_configs = match engine {
        "claude" => vec![("ANTHROPIC_API_KEY", "anthropic")],
        "openai" | "codex" => vec![("OPENAI_API_KEY", "openai")],
        _ => vec![
            ("ANTHROPIC_API_KEY", "anthropic"),
            ("OPENAI_API_KEY", "openai"),
            ("OPENROUTER_API_KEY", "openrouter"),
        ],
    };

    for (env_var, ptype) in &env_configs {
        if let Ok(key) = std::env::var(env_var) {
            if !key.trim().is_empty() {
                let (api_format, base_url) = derive_api_config(ptype);
                return Ok(ApiCredentials {
                    engine_type: ptype.to_string(),
                    api_key: key.trim().to_string(),
                    api_base_url: base_url.to_string(),
                    model: model.to_string(),
                    anthropic_version: "2023-06-01".to_string(),
                    extra_headers: HashMap::new(),
                    force_stream: false,
                    api_format: api_format.to_string(),
                });
            }
        }
    }

    // 3. Try auto-detected providers
    if let Ok(detected) = crate::commands::provider_detect::detect_providers() {
        let provider_type = match engine {
            "claude" => "anthropic",
            "openai" | "codex" => "openai",
            other => other,
        };
        if let Some(dp) = detected.iter().find(|d| d.provider_type == provider_type) {
            let (api_format, _) = derive_api_config(&dp.provider_type);
            return Ok(ApiCredentials {
                engine_type: dp.provider_type.clone(),
                api_key: dp.api_key.clone(),
                api_base_url: dp.api_base_url.clone(),
                model: model.to_string(),
                anthropic_version: "2023-06-01".to_string(),
                extra_headers: HashMap::new(),
                force_stream: false,
                api_format: api_format.to_string(),
            });
        }
    }

    Err(format!(
        "No API provider configured for engine '{}'. Add an {} provider with API key in Settings, \
         set the {} env var, or have a config file available.",
        engine,
        match engine {
            "claude" => "Anthropic",
            "openai" | "codex" => "OpenAI",
            _ => engine,
        },
        match engine {
            "claude" => "ANTHROPIC_API_KEY",
            "openai" | "codex" => "OPENAI_API_KEY",
            _ => "API_KEY",
        }
    ))
}

// ===== Auto Provider Selection =====

pub(crate) fn auto_select_provider_internal() -> Result<(ApiCredentials, SelectedProvider), String>
{
    use crate::commands::settings::derive_api_config;

    let priority: &[&str] = &[
        "anthropic",
        "openai",
        "openrouter",
        "deepseek",
        "groq",
        "mistral",
        "google",
        "custom",
    ];

    // 1. Check configured providers (enabled + healthy first)
    if let Ok(settings) = load_app_settings() {
        let mut candidates: Vec<&AiProvider> = settings
            .providers
            .iter()
            .filter(|p| p.enabled && !p.api_key.is_empty())
            .collect();

        candidates.sort_by(|a, b| {
            let a_health = if a.is_healthy { 0 } else { 1 };
            let b_health = if b.is_healthy { 0 } else { 1 };
            if a_health != b_health {
                return a_health.cmp(&b_health);
            }
            let a_prio = priority
                .iter()
                .position(|&t| t == a.provider_type)
                .unwrap_or(99);
            let b_prio = priority
                .iter()
                .position(|&t| t == b.provider_type)
                .unwrap_or(99);
            a_prio.cmp(&b_prio)
        });

        if let Some(provider) = candidates.first() {
            let (api_format, default_url) = derive_api_config(&provider.provider_type);
            let api_base_url = if provider.api_base_url.is_empty() {
                default_url.to_string()
            } else {
                provider.api_base_url.clone()
            };
            let model = if provider.default_model.is_empty() {
                "auto".to_string()
            } else {
                provider.default_model.clone()
            };

            let creds = ApiCredentials {
                engine_type: provider.provider_type.clone(),
                api_key: provider.api_key.clone(),
                api_base_url: api_base_url.clone(),
                model: model.clone(),
                anthropic_version: if provider.anthropic_version.is_empty() {
                    "2023-06-01".to_string()
                } else {
                    provider.anthropic_version.clone()
                },
                extra_headers: provider.extra_headers.clone(),
                force_stream: provider.force_stream,
                api_format: api_format.to_string(),
            };
            let selected = SelectedProvider {
                provider_id: provider.id.clone(),
                provider_name: provider.name.clone(),
                provider_type: provider.provider_type.clone(),
                api_base_url,
                model,
                api_format: api_format.to_string(),
            };
            return Ok((creds, selected));
        }
    }

    // 2. Environment variables
    let env_checks: &[(&str, &str)] = &[
        ("ANTHROPIC_API_KEY", "anthropic"),
        ("OPENAI_API_KEY", "openai"),
        ("OPENROUTER_API_KEY", "openrouter"),
        ("DEEPSEEK_API_KEY", "deepseek"),
        ("GROQ_API_KEY", "groq"),
        ("GOOGLE_API_KEY", "google"),
    ];

    for (env_var, ptype) in env_checks {
        if let Ok(key) = std::env::var(env_var) {
            if !key.trim().is_empty() {
                let (api_format, default_url) = derive_api_config(ptype);
                let creds = ApiCredentials {
                    engine_type: ptype.to_string(),
                    api_key: key.trim().to_string(),
                    api_base_url: default_url.to_string(),
                    model: "auto".to_string(),
                    anthropic_version: "2023-06-01".to_string(),
                    extra_headers: HashMap::new(),
                    force_stream: false,
                    api_format: api_format.to_string(),
                };
                let selected = SelectedProvider {
                    provider_id: format!("env-{}", ptype),
                    provider_name: format!("env:{}", env_var),
                    provider_type: ptype.to_string(),
                    api_base_url: default_url.to_string(),
                    model: "auto".to_string(),
                    api_format: api_format.to_string(),
                };
                return Ok((creds, selected));
            }
        }
    }

    // 3. Auto-detected providers
    if let Ok(detected) = crate::commands::provider_detect::detect_providers() {
        if let Some(dp) = detected.first() {
            let (api_format, _) = derive_api_config(&dp.provider_type);
            let creds = ApiCredentials {
                engine_type: dp.provider_type.clone(),
                api_key: dp.api_key.clone(),
                api_base_url: dp.api_base_url.clone(),
                model: dp.suggested_model.clone(),
                anthropic_version: "2023-06-01".to_string(),
                extra_headers: HashMap::new(),
                force_stream: false,
                api_format: api_format.to_string(),
            };
            let selected = SelectedProvider {
                provider_id: format!("auto-{}", dp.provider_type),
                provider_name: dp.suggested_name.clone(),
                provider_type: dp.provider_type.clone(),
                api_base_url: dp.api_base_url.clone(),
                model: dp.suggested_model.clone(),
                api_format: api_format.to_string(),
            };
            return Ok((creds, selected));
        }
    }

    Err("No AI provider available. Please configure at least one provider in Settings.".to_string())
}

// ===== Resolve Runtime Config (preview for UI) =====

pub(crate) fn resolve_runtime_config_impl(
    engine: String,
    model: String,
) -> Result<ResolvedRuntimeConfig, String> {
    let mask_key = |key: &str| -> String {
        if key.len() <= 8 {
            "***".to_string()
        } else {
            format!("{}...{}", &key[..4], &key[key.len() - 4..])
        }
    };

    // 1. Try app-level settings
    if let Ok(settings) = load_app_settings() {
        let provider_type = match engine.as_str() {
            "claude" => "anthropic",
            "openai" | "codex" => "openai",
            other => other,
        };

        let provider = settings
            .providers
            .iter()
            .find(|p| p.enabled && p.engine == engine)
            .or_else(|| {
                settings.providers.iter().find(|p| {
                    p.enabled
                        && (p.provider_type == provider_type || p.provider_type == engine.as_str())
                })
            });

        if let Some(provider) = provider {
            if !provider.api_key.is_empty() {
                let api_base_url = if provider.api_base_url.is_empty() {
                    match engine.as_str() {
                        "claude" => "https://api.anthropic.com".to_string(),
                        "openai" | "codex" => "https://api.openai.com".to_string(),
                        _ => provider.api_base_url.clone(),
                    }
                } else {
                    provider.api_base_url.clone()
                };

                let resolved_model =
                    if !provider.default_model.is_empty() && provider.default_model.contains('-') {
                        provider.default_model.clone()
                    } else {
                        resolve_model_name(provider_type, &model)
                    };

                return Ok(ResolvedRuntimeConfig {
                    engine: engine.clone(),
                    model_tier: model,
                    resolved_model,
                    provider_name: provider.name.clone(),
                    provider_type: provider.provider_type.clone(),
                    api_base_url,
                    api_key_preview: mask_key(&provider.api_key),
                    source: "settings".to_string(),
                });
            }
        }
    }

    // 2. Env vars
    let env_configs = match engine.as_str() {
        "claude" => vec![(
            "ANTHROPIC_API_KEY",
            "anthropic",
            "https://api.anthropic.com",
        )],
        "openai" | "codex" => vec![("OPENAI_API_KEY", "openai", "https://api.openai.com")],
        _ => vec![
            (
                "ANTHROPIC_API_KEY",
                "anthropic",
                "https://api.anthropic.com",
            ),
            ("OPENAI_API_KEY", "openai", "https://api.openai.com"),
        ],
    };

    for (env_var, engine_type, base_url) in &env_configs {
        if let Ok(key) = std::env::var(env_var) {
            if !key.trim().is_empty() {
                return Ok(ResolvedRuntimeConfig {
                    engine: engine.clone(),
                    model_tier: model.clone(),
                    resolved_model: resolve_model_name(engine_type, &model),
                    provider_name: format!("env:{}", env_var),
                    provider_type: engine_type.to_string(),
                    api_base_url: base_url.to_string(),
                    api_key_preview: mask_key(key.trim()),
                    source: format!("env:{}", env_var),
                });
            }
        }
    }

    // 3. Auto-detected
    if let Ok(detected) = crate::commands::provider_detect::detect_providers() {
        let provider_type = match engine.as_str() {
            "claude" => "anthropic",
            "openai" | "codex" => "openai",
            other => other,
        };
        if let Some(dp) = detected.iter().find(|d| d.provider_type == provider_type) {
            return Ok(ResolvedRuntimeConfig {
                engine: engine.clone(),
                model_tier: model.clone(),
                resolved_model: resolve_model_name(provider_type, &model),
                provider_name: dp.suggested_name.clone(),
                provider_type: dp.provider_type.clone(),
                api_base_url: dp.api_base_url.clone(),
                api_key_preview: mask_key(&dp.api_key),
                source: dp.source.clone(),
            });
        }
    }

    Err(format!("No provider configured for engine '{}'", engine))
}

// ===== Engine Binary Resolution =====

pub fn resolve_engine_binary(engine: &str) -> Result<String, String> {
    let candidates: &[&str] = match engine {
        "claude" => &["claude"],
        "codex" => &["codex"],
        "opencode" => &["opencode"],
        _ => return Err(format!("Unknown engine: {}", engine)),
    };

    for candidate in candidates {
        if let Some(path) = find_binary(candidate) {
            return Ok(path);
        }
    }

    let install_hint = match engine {
        "claude" => "npm install -g @anthropic-ai/claude-code",
        "codex" => "npm install -g @openai/codex",
        "opencode" => "go install github.com/opencode-ai/opencode@latest",
        _ => "See documentation",
    };

    Err(format!(
        "{} CLI not found in PATH. Install with: {}",
        engine, install_hint
    ))
}

pub fn find_binary(name: &str) -> Option<String> {
    use super::silent_command;

    #[cfg(target_os = "windows")]
    {
        silent_command("where")
            .arg(name)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                let out = String::from_utf8_lossy(&o.stdout);
                out.lines().next().map(|l| l.trim().to_string())
            })
    }

    #[cfg(not(target_os = "windows"))]
    {
        silent_command("which")
            .arg(name)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                let out = String::from_utf8_lossy(&o.stdout);
                out.lines().next().map(|l| l.trim().to_string())
            })
    }
}
