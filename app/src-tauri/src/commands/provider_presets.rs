use serde::{Deserialize, Serialize};
use tauri::command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOption {
    pub id: String,
    pub name: String,
    pub tier: String,
    pub context_window: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPreset {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub default_url: String,
    pub icon: String,
    pub icon_color: String,
    pub models: Vec<ModelOption>,
    pub description: String,
}

fn built_in_presets() -> Vec<ProviderPreset> {
    vec![
        ProviderPreset {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            provider_type: "anthropic".to_string(),
            default_url: "https://api.anthropic.com".to_string(),
            icon: "brain".to_string(),
            icon_color: "#D97706".to_string(),
            description: "Claude models - best for reasoning and coding".to_string(),
            models: vec![
                ModelOption {
                    id: "claude-opus-4-20250514".to_string(),
                    name: "Claude Opus 4".to_string(),
                    tier: "opus".to_string(),
                    context_window: 200000,
                },
                ModelOption {
                    id: "claude-sonnet-4-20250514".to_string(),
                    name: "Claude Sonnet 4".to_string(),
                    tier: "sonnet".to_string(),
                    context_window: 200000,
                },
                ModelOption {
                    id: "claude-3-5-haiku-20241022".to_string(),
                    name: "Claude 3.5 Haiku".to_string(),
                    tier: "haiku".to_string(),
                    context_window: 200000,
                },
            ],
        },
        ProviderPreset {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            provider_type: "openai".to_string(),
            default_url: "https://api.openai.com/v1".to_string(),
            icon: "sparkles".to_string(),
            icon_color: "#10B981".to_string(),
            description: "GPT models - versatile general purpose".to_string(),
            models: vec![
                ModelOption {
                    id: "gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                    tier: "sonnet".to_string(),
                    context_window: 128000,
                },
                ModelOption {
                    id: "gpt-4o-mini".to_string(),
                    name: "GPT-4o Mini".to_string(),
                    tier: "haiku".to_string(),
                    context_window: 128000,
                },
                ModelOption {
                    id: "o3".to_string(),
                    name: "o3".to_string(),
                    tier: "opus".to_string(),
                    context_window: 200000,
                },
                ModelOption {
                    id: "o4-mini".to_string(),
                    name: "o4-mini".to_string(),
                    tier: "sonnet".to_string(),
                    context_window: 200000,
                },
            ],
        },
        ProviderPreset {
            id: "openrouter".to_string(),
            name: "OpenRouter".to_string(),
            provider_type: "openrouter".to_string(),
            default_url: "https://openrouter.ai/api/v1".to_string(),
            icon: "route".to_string(),
            icon_color: "#6366F1".to_string(),
            description: "Access 200+ models through one API".to_string(),
            models: vec![
                ModelOption {
                    id: "anthropic/claude-sonnet-4-20250514".to_string(),
                    name: "Claude Sonnet 4".to_string(),
                    tier: "sonnet".to_string(),
                    context_window: 200000,
                },
                ModelOption {
                    id: "openai/gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                    tier: "sonnet".to_string(),
                    context_window: 128000,
                },
                ModelOption {
                    id: "google/gemini-2.5-pro".to_string(),
                    name: "Gemini 2.5 Pro".to_string(),
                    tier: "opus".to_string(),
                    context_window: 1000000,
                },
                ModelOption {
                    id: "deepseek/deepseek-r1".to_string(),
                    name: "DeepSeek R1".to_string(),
                    tier: "opus".to_string(),
                    context_window: 128000,
                },
            ],
        },
        ProviderPreset {
            id: "deepseek".to_string(),
            name: "DeepSeek".to_string(),
            provider_type: "deepseek".to_string(),
            default_url: "https://api.deepseek.com/v1".to_string(),
            icon: "search".to_string(),
            icon_color: "#3B82F6".to_string(),
            description: "Cost-effective reasoning and coding models".to_string(),
            models: vec![
                ModelOption {
                    id: "deepseek-chat".to_string(),
                    name: "DeepSeek V3".to_string(),
                    tier: "sonnet".to_string(),
                    context_window: 128000,
                },
                ModelOption {
                    id: "deepseek-reasoner".to_string(),
                    name: "DeepSeek R1".to_string(),
                    tier: "opus".to_string(),
                    context_window: 128000,
                },
            ],
        },
        ProviderPreset {
            id: "google".to_string(),
            name: "Google Gemini".to_string(),
            provider_type: "google".to_string(),
            default_url: "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
            icon: "gem".to_string(),
            icon_color: "#F59E0B".to_string(),
            description: "Gemini models with massive context windows".to_string(),
            models: vec![
                ModelOption {
                    id: "gemini-2.5-pro".to_string(),
                    name: "Gemini 2.5 Pro".to_string(),
                    tier: "opus".to_string(),
                    context_window: 1000000,
                },
                ModelOption {
                    id: "gemini-2.5-flash".to_string(),
                    name: "Gemini 2.5 Flash".to_string(),
                    tier: "sonnet".to_string(),
                    context_window: 1000000,
                },
            ],
        },
        ProviderPreset {
            id: "groq".to_string(),
            name: "Groq".to_string(),
            provider_type: "groq".to_string(),
            default_url: "https://api.groq.com/openai/v1".to_string(),
            icon: "zap".to_string(),
            icon_color: "#EF4444".to_string(),
            description: "Ultra-fast inference for open-source models".to_string(),
            models: vec![
                ModelOption {
                    id: "llama-3.3-70b-versatile".to_string(),
                    name: "Llama 3.3 70B".to_string(),
                    tier: "sonnet".to_string(),
                    context_window: 128000,
                },
                ModelOption {
                    id: "llama-3.1-8b-instant".to_string(),
                    name: "Llama 3.1 8B".to_string(),
                    tier: "haiku".to_string(),
                    context_window: 128000,
                },
            ],
        },
        ProviderPreset {
            id: "mistral".to_string(),
            name: "Mistral".to_string(),
            provider_type: "mistral".to_string(),
            default_url: "https://api.mistral.ai/v1".to_string(),
            icon: "wind".to_string(),
            icon_color: "#8B5CF6".to_string(),
            description: "European AI - efficient multilingual models".to_string(),
            models: vec![
                ModelOption {
                    id: "mistral-large-latest".to_string(),
                    name: "Mistral Large".to_string(),
                    tier: "opus".to_string(),
                    context_window: 128000,
                },
                ModelOption {
                    id: "mistral-small-latest".to_string(),
                    name: "Mistral Small".to_string(),
                    tier: "haiku".to_string(),
                    context_window: 128000,
                },
            ],
        },
        ProviderPreset {
            id: "custom".to_string(),
            name: "Custom Gateway".to_string(),
            provider_type: "custom".to_string(),
            default_url: String::new(),
            icon: "settings".to_string(),
            icon_color: "#6B7280".to_string(),
            description: "Any OpenAI-compatible API endpoint".to_string(),
            models: vec![],
        },
    ]
}

#[command]
pub fn get_provider_presets() -> Vec<ProviderPreset> {
    built_in_presets()
}
