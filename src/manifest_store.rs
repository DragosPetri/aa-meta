use std::path::Path;

use sha2::{Digest, Sha256};

use crate::config::{AppConfig, ToolConfig};
use crate::error::AttachMetaError;
use crate::protocol::manifest::{Manifest, parse_manifest};
use crate::protocol::version;
use crate::schema;

pub struct VerifiedManifest {
    pub manifest: Manifest,
}

pub fn load_verified(
    tool: &ToolConfig,
    config: &mut AppConfig,
    config_path: &Path,
) -> std::result::Result<VerifiedManifest, AttachMetaError> {
    let manifest_path = tool.manifest_path.as_ref().ok_or_else(|| {
        AttachMetaError::ManifestError("tool has no manifest_path — run 'init' first".to_string())
    })?;

    let content = std::fs::read_to_string(manifest_path).map_err(|e| {
        AttachMetaError::ManifestError(format!(
            "failed to read manifest at '{}': {e}",
            manifest_path.display()
        ))
    })?;

    let new_hash = sha256_hex(&content);
    let stored_hash = tool.manifest_sha256.as_deref();
    let hash_changed = stored_hash != Some(new_hash.as_str());

    let manifest_json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AttachMetaError::ManifestError(format!("manifest is not valid JSON: {e}")))?;

    let manifest =
        parse_manifest(&content).map_err(|e| AttachMetaError::ManifestError(e.to_string()))?;

    if hash_changed {
        schema::validate_manifest(&manifest_json)?;

        let binary = tool.binary.as_deref().unwrap_or("<unknown>");
        version::check_major_match(&manifest.protocol_version, binary)
            .map_err(|e| AttachMetaError::ManifestError(e.to_string()))?;

        if let Some(tool_entry) = config.tools.iter_mut().find(|t| t.name == tool.name) {
            tool_entry.manifest_sha256 = Some(new_hash.clone());
            if let Err(e) = config.save(config_path) {
                eprintln!("attach-meta: warning: failed to persist updated hash: {e}");
            }
        }
    }

    Ok(VerifiedManifest { manifest })
}

pub fn sha256_hex(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}
