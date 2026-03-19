use crate::commands::library;
use crate::engine;
use crate::models::*;
use std::path::PathBuf;
use tauri::command;

#[command]
pub fn analyze_seed(prompt: String) -> Result<SeedAnalysis, String> {
    Ok(engine::bootstrap::analyze_seed(&prompt))
}

#[command]
pub fn bootstrap(prompt: String, output_dir: String) -> Result<FactoryConfig, String> {
    let config = engine::bootstrap::build_config(&prompt);

    // Save config to output dir
    let dir = PathBuf::from(&output_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create output dir: {}", e))?;

    let yaml =
        serde_yaml::to_string(&config).map_err(|e| format!("YAML serialize error: {}", e))?;
    std::fs::write(dir.join("company.yaml"), &yaml).map_err(|e| format!("Write error: {}", e))?;

    // Auto-generate all project files immediately
    let templates_dir = dir.join("templates");
    engine::generator::generate_all(&config, &dir, &templates_dir)?;

    // Register project in the global registry so Dashboard can find it
    library::register_project(&config.company.name, &output_dir)?;

    Ok(config)
}

#[command]
pub fn generate(config_path: String) -> Result<GenerateResult, String> {
    let path = PathBuf::from(&config_path);
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read config: {}", e))?;
    let config: FactoryConfig =
        serde_yaml::from_str(&content).map_err(|e| format!("YAML parse error: {}", e))?;

    let fallback = PathBuf::from(".");
    let output_dir = path.parent().unwrap_or(&fallback);
    let templates_dir = output_dir.join("templates");

    engine::generator::generate_all(&config, output_dir, &templates_dir)
}

#[command]
pub fn validate_config(config: FactoryConfig) -> Vec<String> {
    engine::guardrails::validate_config_guardrails(&config.guardrails)
}

#[command]
pub fn save_config(config: FactoryConfig, path: String) -> Result<bool, String> {
    let yaml =
        serde_yaml::to_string(&config).map_err(|e| format!("YAML serialize error: {}", e))?;
    std::fs::write(&path, &yaml).map_err(|e| format!("Write error: {}", e))?;
    Ok(true)
}
