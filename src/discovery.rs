use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DiscoveryResponse {
    pub protocol_version: String,
    pub tool_name: String,
    pub tool_version: String,
    #[serde(default)]
    pub description: Option<String>,
    pub commands: HashMap<String, CommandEntry>,
    #[serde(default)]
    pub settings: Vec<SettingEntry>,
}

#[derive(Debug, Deserialize)]
pub struct CommandEntry {
    pub argv: Vec<String>,
    #[serde(default = "default_supported")]
    pub supported: bool,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SettingEntry {
    pub key: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub default: Option<String>,
}

fn default_supported() -> bool {
    true
}

pub fn run_discovery(binary: &str) -> Result<DiscoveryResponse> {
    let output = std::process::Command::new(binary)
        .args(["discovery", "--json"])
        .output()
        .with_context(|| format!("failed to run '{binary} discovery --json' — is '{binary}' on PATH?"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "'{binary} discovery --json' exited with {}\n{stderr}",
            output.status
        );
    }

    let response: DiscoveryResponse = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("'{binary} discovery --json' returned invalid JSON"))?;

    check_protocol_version(&response.protocol_version, binary)?;

    Ok(response)
}

fn check_protocol_version(version: &str, binary: &str) -> Result<()> {
    let tool_major = version.split('.').next().unwrap_or("0");
    let our_major = env!("CARGO_PKG_VERSION").split('.').next().unwrap_or("0");

    if tool_major != our_major {
        bail!(
            "protocol version mismatch: '{binary}' reports '{version}', \
             attach-meta is '{our_major}.x' — major versions must match"
        );
    }

    Ok(())
}
