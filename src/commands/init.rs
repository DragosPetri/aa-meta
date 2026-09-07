use std::path::Path;

use crate::config::{AppConfig, ToolConfig};
use crate::error::AttachMetaError;
use crate::manifest_store;
use crate::prompt::{format_config_prompt, Prompter};
use crate::protocol::manifest::CommandName;
use crate::protocol::responses::{Config, InitResponse, Severity, ToolConfigResponse};
use crate::protocol::version;
use crate::schema;
use crate::transport;

pub fn run_init(
    binary: &str,
    no_interactive: bool,
    prompter: &dyn Prompter,
    config: &mut AppConfig,
    config_path: &Path,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    // Step 1: call <binary> attach-manifest
    let output = std::process::Command::new(binary)
        .arg("attach-manifest")
        .output()
        .map_err(|e| {
            AttachMetaError::TransportError(format!(
                "failed to run '{binary} attach-manifest': {e}"
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AttachMetaError::TransportError(format!(
            "'{binary} attach-manifest' exited with {}: {stderr}",
            output.status
        )));
    }

    // Step 2: read manifest path from stdout
    let manifest_path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if manifest_path_str.is_empty() {
        return Err(AttachMetaError::TransportError(
            "attach-manifest produced empty stdout — expected a file path".to_string(),
        ));
    }

    let manifest_path = std::path::PathBuf::from(&manifest_path_str);
    if !manifest_path.exists() {
        return Err(AttachMetaError::TransportError(format!(
            "manifest path '{}' does not exist",
            manifest_path.display()
        )));
    }

    // Step 3: read and parse manifest
    let content = std::fs::read_to_string(&manifest_path).map_err(|e| {
        AttachMetaError::ManifestError(format!(
            "failed to read manifest at '{}': {e}",
            manifest_path.display()
        ))
    })?;

    let manifest_json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AttachMetaError::ManifestError(format!("manifest is not valid JSON: {e}")))?;

    // Step 4: validate against meta-schema
    schema::validate_manifest(&manifest_json)?;

    // Step 5: check protocol_version major match
    let manifest = crate::protocol::manifest::parse_manifest(&content)
        .map_err(|e| AttachMetaError::ManifestError(e.to_string()))?;

    version::check_major_match(&manifest.protocol_version, binary)
        .map_err(|e| AttachMetaError::ManifestError(e.to_string()))?;

    // Step 6: write to .attach-meta.toml
    let hash = manifest_store::sha256_hex(&content);

    // Find or create tool entry
    let tool_name = binary.rsplit('/').next().unwrap_or(binary).to_string();
    let existing = config.tools.iter_mut().find(|t| t.name == tool_name);
    match existing {
        Some(tool) => {
            tool.binary = Some(binary.to_string());
            tool.manifest_path = Some(manifest_path.clone());
            tool.manifest_sha256 = Some(hash);
        }
        None => {
            config.tools.push(ToolConfig {
                name: tool_name.clone(),
                binary: Some(binary.to_string()),
                manifest_path: Some(manifest_path.clone()),
                manifest_sha256: Some(hash),
                settings: Default::default(),
            });
        }
    }

    if config.meta.default_tool.is_none() {
        config.meta.default_tool = Some(tool_name.clone());
    }

    config
        .save(config_path)
        .map_err(|e| AttachMetaError::InternalError(format!("failed to save config: {e}")))?;

    // Step 7: interactive config session
    let mut missing_fields = Vec::new();
    let config_complete;

    if let Some(tcg_mapping) = manifest.get_command(CommandName::ToolConfigGet) {
        match transport::invoke(tcg_mapping, &[]) {
            Ok(resp) => {
                if let Ok(tcr) = serde_json::from_value::<ToolConfigResponse>(resp) {
                    // Determine missing required fields
                    for cfg in &tcr.configs {
                        if cfg.required && cfg.default.is_null() {
                            missing_fields.push(cfg.field_name.clone());
                        }
                    }

                    if !no_interactive {
                        run_interactive_config(
                            &tcr.configs,
                            &manifest,
                            prompter,
                            &mut missing_fields,
                        );
                    }
                }
            }
            Err(_) => {
                // tool-config-get failed — config not complete
            }
        }
    }

    config_complete = missing_fields.is_empty();

    let init_response = InitResponse {
        ok: true,
        message: format!("Initialized tool '{tool_name}' successfully."),
        severity: Severity::Info,
        config_complete,
        missing_fields,
    };

    serde_json::to_value(&init_response).map_err(|e| {
        AttachMetaError::InternalError(format!("failed to serialize init response: {e}"))
    })
}

fn run_interactive_config(
    configs: &[Config],
    manifest: &crate::protocol::manifest::Manifest,
    prompter: &dyn Prompter,
    missing_fields: &mut Vec<String>,
) {
    let tcs_mapping = match manifest.get_command(CommandName::ToolConfigSet) {
        Some(m) => m,
        None => return,
    };

    // Required fields
    for cfg in configs.iter().filter(|c| c.required) {
        eprintln!("{}", format_config_prompt(cfg));

        let value = if !cfg.default.is_null() {
            if prompter.confirm(&format!("  Use default '{}'?", cfg.default), true) {
                cfg.default.to_string().trim_matches('"').to_string()
            } else {
                match prompter.prompt_line("  Enter value: ") {
                    Some(v) => v,
                    None => continue,
                }
            }
        } else {
            match prompter.prompt_line("  Enter value: ") {
                Some(v) => v,
                None => continue,
            }
        };

        let args = vec![cfg.field_name.clone(), value];
        if transport::invoke(tcs_mapping, &args).is_ok() {
            missing_fields.retain(|f| f != &cfg.field_name);
        }
    }

    // Optional fields
    let optional: Vec<&Config> = configs.iter().filter(|c| !c.required).collect();
    if !optional.is_empty() && prompter.confirm("Configure optional fields?", false) {
        for cfg in optional {
            eprintln!("{}", format_config_prompt(cfg));
            if let Some(value) = prompter.prompt_line("  Enter value (or empty to skip): ") {
                let args = vec![cfg.field_name.clone(), value];
                let _ = transport::invoke(tcs_mapping, &args);
            }
        }
    }
}
