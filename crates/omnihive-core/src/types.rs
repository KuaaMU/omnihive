//! Core types shared between the desktop app and CLI.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailConfig {
    pub forbidden: Vec<String>,
    #[serde(default = "default_workspace")]
    pub workspace: String,
    #[serde(default)]
    pub require_critic_review: bool,
}

fn default_workspace() -> String {
    "projects/".to_string()
}

impl Default for GuardrailConfig {
    fn default() -> Self {
        Self {
            forbidden: vec![],
            workspace: default_workspace(),
            require_critic_review: false,
        }
    }
}
