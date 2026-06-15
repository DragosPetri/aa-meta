use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub meta: MetaConfig,
    #[serde(default)]
    pub tools: Vec<ToolConfig>,
}

#[derive(Debug, Deserialize, Default)]
pub struct MetaConfig {
    pub default_tool: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ToolConfig {
    pub name: String,
    pub binary: String,
    #[serde(default)]
    pub settings: HashMap<String, String>,
}

impl AppConfig {
    pub fn find_tool(&self, name: &str) -> Option<&ToolConfig> {
        self.tools.iter().find(|t| t.name == name)
    }
}

pub fn load_config(override_path: Option<PathBuf>) -> Result<AppConfig> {
    let path = override_path
        .or_else(find_project_config)
        .unwrap_or_else(user_config_path);

    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;

    toml::from_str(&contents)
        .with_context(|| format!("failed to parse config file: {}", path.display()))
}

/// Walk up from cwd looking for `.attach-meta.toml`.
fn find_project_config() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".attach-meta.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn user_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("attach-meta")
        .join("config.toml")
}
