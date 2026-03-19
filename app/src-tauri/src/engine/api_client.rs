use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::BufRead;
use std::time::Duration;

// ===== Configurable API Call =====

pub struct ApiCallConfig {
    pub api_key: String,
    pub api_base_url: String,
    pub model: String,
    pub system_prompt: String,
    pub user_message: String,
    pub timeout_secs: u32,
    pub anthropic_version: String,
    pub extra_headers: HashMap<String, String>,
    pub force_stream: bool,
    pub api_format: String, // "anthropic" | "claude-code" | "openai"
}

impl Default for ApiCallConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base_url: "https://api.anthropic.com".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            system_prompt: String::new(),
            user_message: String::new(),
            timeout_secs: 1800,
            anthropic_version: "2023-06-01".to_string(),
            extra_headers: HashMap::new(),
            force_stream: false,
            api_format: "anthropic".to_string(),
        }
    }
}

// ===== Anthropic API Types =====

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: serde_json::Value,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(default)]
    text: Option<String>,
    #[serde(rename = "type")]
    content_type: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

// ===== OpenAI API Types =====

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ApiMessage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: OpenAiUsage,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

// ===== SSE Streaming Types =====

#[derive(Debug, Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    delta: Option<StreamDelta>,
    #[serde(default)]
    message: Option<StreamMessageEnd>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    #[serde(rename = "type", default)]
    delta_type: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamMessageEnd {
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

// ===== Public Types =====

pub struct CycleResponse {
    pub text: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// ===== Unified API Call =====

pub fn call_api(config: &ApiCallConfig) -> Result<CycleResponse, String> {
    let format = config.api_format.as_str();
    match format {
        "openai" => call_openai(
            &config.api_key,
            &config.api_base_url,
            &config.model,
            &config.system_prompt,
            &config.user_message,
            config.timeout_secs,
        ),
        _ => {
            if config.force_stream {
                call_anthropic_streaming(config)
            } else {
                call_anthropic_configurable(config)
            }
        }
    }
}

// ===== Anthropic API (configurable) =====

fn call_anthropic_configurable(config: &ApiCallConfig) -> Result<CycleResponse, String> {
    let url = format!("{}/v1/messages", config.api_base_url.trim_end_matches('/'));
    let resolved_model = resolve_anthropic_model(&config.model);

    let system_value = build_system_value(&config.system_prompt, &config.api_format);

    let body = AnthropicRequest {
        model: resolved_model,
        max_tokens: 4096,
        system: system_value,
        messages: vec![ApiMessage {
            role: "user".to_string(),
            content: config.user_message.clone(),
        }],
        stream: None,
    };

    let agent = ureq::AgentBuilder::new()
        .timeout_read(Duration::from_secs(config.timeout_secs as u64))
        .timeout_write(Duration::from_secs(30))
        .build();

    let mut req = agent
        .post(&url)
        .set("x-api-key", &config.api_key)
        .set("anthropic-version", &config.anthropic_version)
        .set("content-type", "application/json");

    // Apply extra headers
    for (key, value) in &config.extra_headers {
        req = req.set(key, value);
    }

    let result = req.send_json(&body);

    match result {
        Ok(resp) => {
            let data: AnthropicResponse = resp
                .into_json()
                .map_err(|e| format!("Failed to parse Anthropic response: {}", e))?;

            let text = data
                .content
                .into_iter()
                .filter_map(|c| {
                    if c.content_type == "text" {
                        c.text
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("");

            Ok(CycleResponse {
                text,
                input_tokens: data.usage.input_tokens,
                output_tokens: data.usage.output_tokens,
            })
        }
        Err(ureq::Error::Status(code, resp)) => {
            let error_body = resp.into_string().unwrap_or_default();
            let preview = truncate(&error_body, 2000);
            Err(format!("Anthropic API error (HTTP {}): {}", code, preview))
        }
        Err(e) => Err(format!("Anthropic request failed: {}", e)),
    }
}

// ===== Anthropic Streaming API =====

fn call_anthropic_streaming(config: &ApiCallConfig) -> Result<CycleResponse, String> {
    let url = format!("{}/v1/messages", config.api_base_url.trim_end_matches('/'));
    let resolved_model = resolve_anthropic_model(&config.model);

    let system_value = build_system_value(&config.system_prompt, &config.api_format);

    let body = AnthropicRequest {
        model: resolved_model,
        max_tokens: 4096,
        system: system_value,
        messages: vec![ApiMessage {
            role: "user".to_string(),
            content: config.user_message.clone(),
        }],
        stream: Some(true),
    };

    let agent = ureq::AgentBuilder::new()
        .timeout_read(Duration::from_secs(config.timeout_secs as u64))
        .timeout_write(Duration::from_secs(30))
        .build();

    let mut req = agent
        .post(&url)
        .set("x-api-key", &config.api_key)
        .set("anthropic-version", &config.anthropic_version)
        .set("content-type", "application/json");

    for (key, value) in &config.extra_headers {
        req = req.set(key, value);
    }

    let result = req.send_json(&body);

    match result {
        Ok(resp) => parse_sse_stream(resp),
        Err(ureq::Error::Status(code, resp)) => {
            let error_body = resp.into_string().unwrap_or_default();
            let preview = truncate(&error_body, 2000);
            Err(format!(
                "Anthropic Streaming API error (HTTP {}): {}",
                code, preview
            ))
        }
        Err(e) => Err(format!("Anthropic streaming request failed: {}", e)),
    }
}

fn parse_sse_stream(resp: ureq::Response) -> Result<CycleResponse, String> {
    let reader = std::io::BufReader::new(resp.into_reader());
    let mut full_text = String::new();
    let mut input_tokens: u32 = 0;
    let mut output_tokens: u32 = 0;

    for line_result in reader.lines() {
        let line = line_result.map_err(|e| format!("Stream read error: {}", e))?;

        // SSE format: lines starting with "data: "
        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                break;
            }

            if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
                match event.event_type.as_str() {
                    "content_block_delta" => {
                        if let Some(delta) = &event.delta {
                            if delta.delta_type == "text_delta" {
                                if let Some(ref text) = delta.text {
                                    full_text.push_str(text);
                                }
                            }
                        }
                    }
                    "message_delta" => {
                        // message_delta may contain final usage in delta
                    }
                    "message_start" => {
                        // message_start may contain usage.input_tokens
                        if let Some(msg) = &event.message {
                            if let Some(usage) = &msg.usage {
                                input_tokens = usage.input_tokens;
                                output_tokens = usage.output_tokens;
                            }
                        }
                    }
                    "message_stop" => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    // Estimate output tokens from text length if not provided
    if output_tokens == 0 && !full_text.is_empty() {
        output_tokens = (full_text.len() as u32) / 4;
    }

    Ok(CycleResponse {
        text: full_text,
        input_tokens,
        output_tokens,
    })
}

// ===== Legacy call_anthropic (backward compat) =====

pub fn call_anthropic(
    api_key: &str,
    api_base_url: &str,
    model: &str,
    system_prompt: &str,
    user_message: &str,
    timeout_secs: u32,
) -> Result<CycleResponse, String> {
    let config = ApiCallConfig {
        api_key: api_key.to_string(),
        api_base_url: api_base_url.to_string(),
        model: model.to_string(),
        system_prompt: system_prompt.to_string(),
        user_message: user_message.to_string(),
        timeout_secs,
        ..Default::default()
    };
    call_anthropic_configurable(&config)
}

// ===== OpenAI API =====

pub fn call_openai(
    api_key: &str,
    api_base_url: &str,
    model: &str,
    system_prompt: &str,
    user_message: &str,
    timeout_secs: u32,
) -> Result<CycleResponse, String> {
    let url = format!("{}/v1/chat/completions", api_base_url.trim_end_matches('/'));

    let body = OpenAiRequest {
        model: model.to_string(),
        max_tokens: 4096,
        messages: vec![
            ApiMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ApiMessage {
                role: "user".to_string(),
                content: user_message.to_string(),
            },
        ],
    };

    let agent = ureq::AgentBuilder::new()
        .timeout_read(Duration::from_secs(timeout_secs as u64))
        .timeout_write(Duration::from_secs(30))
        .build();

    let result = agent
        .post(&url)
        .set("Authorization", &format!("Bearer {}", api_key))
        .set("content-type", "application/json")
        .send_json(&body);

    match result {
        Ok(resp) => {
            let data: OpenAiResponse = resp
                .into_json()
                .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;

            let text = data
                .choices
                .first()
                .and_then(|c| c.message.content.clone())
                .unwrap_or_default();

            Ok(CycleResponse {
                text,
                input_tokens: data.usage.prompt_tokens,
                output_tokens: data.usage.completion_tokens,
            })
        }
        Err(ureq::Error::Status(code, resp)) => {
            let error_body = resp.into_string().unwrap_or_default();
            let preview = truncate(&error_body, 2000);
            Err(format!("OpenAI API error (HTTP {}): {}", code, preview))
        }
        Err(e) => Err(format!("OpenAI request failed: {}", e)),
    }
}

// ===== System Value Builder =====

fn build_system_value(system_prompt: &str, api_format: &str) -> serde_json::Value {
    match api_format {
        "claude-code" => {
            // Claude Code compatible: system as array of content blocks
            serde_json::json!([{"type": "text", "text": system_prompt}])
        }
        _ => {
            // Standard Anthropic: system as plain string
            serde_json::Value::String(system_prompt.to_string())
        }
    }
}

// ===== Model Resolution =====

fn resolve_anthropic_model(model: &str) -> String {
    // If model already looks like a full model ID (contains dashes), pass through directly
    if model.starts_with("claude-") || model.contains('/') {
        return model.to_string();
    }
    // Map tier names to latest model IDs
    match model {
        "opus" => "claude-opus-4-20250514".to_string(),
        "sonnet" => "claude-sonnet-4-20250514".to_string(),
        "haiku" => "claude-3-5-haiku-20241022".to_string(),
        other => other.to_string(),
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- resolve_anthropic_model ---

    #[test]
    fn test_resolve_anthropic_model_opus() {
        assert_eq!(resolve_anthropic_model("opus"), "claude-opus-4-20250514");
    }

    #[test]
    fn test_resolve_anthropic_model_sonnet() {
        assert_eq!(
            resolve_anthropic_model("sonnet"),
            "claude-sonnet-4-20250514"
        );
    }

    #[test]
    fn test_resolve_anthropic_model_haiku() {
        assert_eq!(
            resolve_anthropic_model("haiku"),
            "claude-3-5-haiku-20241022"
        );
    }

    #[test]
    fn test_resolve_anthropic_model_passthrough_full_id() {
        let model = "claude-sonnet-4-20250514";
        assert_eq!(resolve_anthropic_model(model), model);
    }

    #[test]
    fn test_resolve_anthropic_model_passthrough_custom() {
        assert_eq!(
            resolve_anthropic_model("my-custom-model"),
            "my-custom-model"
        );
    }

    #[test]
    fn test_resolve_anthropic_model_unknown_tier() {
        assert_eq!(resolve_anthropic_model("gpt4"), "gpt4");
    }

    // --- build_system_value ---

    #[test]
    fn test_build_system_value_anthropic() {
        let val = build_system_value("test prompt", "anthropic");
        assert_eq!(val, serde_json::Value::String("test prompt".to_string()));
    }

    #[test]
    fn test_build_system_value_claude_code() {
        let val = build_system_value("test prompt", "claude-code");
        assert!(val.is_array());
        let arr = val.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["type"], "text");
        assert_eq!(arr[0]["text"], "test prompt");
    }

    // --- truncate ---

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        let result = truncate("hello world!", 5);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn test_truncate_exact_length() {
        assert_eq!(truncate("exact", 5), "exact");
    }

    // --- ApiCallConfig default ---

    #[test]
    fn test_api_call_config_default() {
        let config = ApiCallConfig::default();
        assert_eq!(config.api_base_url, "https://api.anthropic.com");
        assert_eq!(config.model, "claude-sonnet-4-20250514");
        assert_eq!(config.timeout_secs, 1800);
        assert_eq!(config.api_format, "anthropic");
        assert!(!config.force_stream);
    }
}
