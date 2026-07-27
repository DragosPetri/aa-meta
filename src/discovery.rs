use std::collections::HashMap;

use anyhow::{Context, Result, bail};
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
        .args(["--json", "discovery"])
        .output()
        .with_context(|| {
            format!("failed to run '{binary} discovery --json' — is '{binary}' on PATH?")
        })?;

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
    let our_version = env!("CARGO_PKG_VERSION");
    let breaking = |v: &str| {
        let mut parts = v.split('.');
        let major = parts.next().unwrap_or("0");
        if major == "0" {
            // semver: 0.x — minor is breaking
            format!("0.{}", parts.next().unwrap_or("0"))
        } else {
            major.to_string()
        }
    };

    let tool_breaking = breaking(version);
    let our_breaking = breaking(our_version);

    if tool_breaking != our_breaking {
        bail!(
            "protocol version mismatch: '{binary}' reports '{version}', \
             attach-meta is '{our_version}' — breaking segment must match \
             (got '{tool_breaking}', need '{our_breaking}')"
        );
    }

    Ok(())
}
