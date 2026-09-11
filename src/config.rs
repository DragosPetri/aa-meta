use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct AppConfig {
    #[serde(default)]
    pub meta: MetaConfig,
    #[serde(default)]
    pub tools: Vec<ToolConfig>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct MetaConfig {
    pub default_tool: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ToolConfig {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_sha256: Option<String>,
    #[serde(default)]
    pub settings: HashMap<String, String>,
}

impl AppConfig {
    pub fn find_tool(&self, name: &str) -> Option<&ToolConfig> {
        self.tools.iter().find(|t| t.name == name)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).context("failed to serialize config")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(path, content)
            .with_context(|| format!("failed to write config to {}", path.display()))
    }
}

pub fn load_config() -> Result<(AppConfig, PathBuf)> {
    let path = find_project_config().unwrap_or_else(default_project_config_path);

    if !path.exists() {
        return Ok((AppConfig::default(), path));
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;

    let config: AppConfig = toml::from_str(&contents)
        .with_context(|| format!("failed to parse config file: {}", path.display()))?;

    Ok((config, path))
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

fn default_project_config_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".attach-meta.toml")
}

pub fn try_load_manifest(
    config: &mut AppConfig,
    config_path: &Path,
) -> Option<crate::protocol::manifest::Manifest> {
    let tool = config
        .meta
        .default_tool
        .as_deref()
        .and_then(|name| config.find_tool(name).cloned())?;
    crate::manifest_store::load_verified(&tool, config, config_path)
        .ok()
        .map(|v| v.manifest)
}

pub fn user_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("attach-meta")
        .join("config.toml")
}
