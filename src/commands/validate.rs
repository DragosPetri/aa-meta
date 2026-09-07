use crate::commands::CommandContext;
use crate::error::AttachMetaError;
use crate::protocol::manifest::CommandName;
use crate::protocol::responses::ValidationResponse;
use crate::transport;

pub fn run(
    _cmd: CommandName,
    positionals: &[String],
    _flags: &serde_json::Value,
    ctx: &CommandContext,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    let mapping = ctx
        .manifest
        .get_command(CommandName::Validate)
        .ok_or_else(|| {
            AttachMetaError::ManifestError("command 'validate' not in manifest".to_string())
        })?;

    let response = transport::invoke(mapping, &[])?;

    if positionals.is_empty() {
        return Ok(response);
    }

    // Filter errors/warnings by path prefix
    let mut vr: ValidationResponse = serde_json::from_value(response)
        .map_err(|e| AttachMetaError::TransportError(format!("invalid validate response: {e}")))?;

    vr.errors = filter_by_prefix(&vr.errors, positionals);
    vr.warnings = filter_by_prefix(&vr.warnings, positionals);

    serde_json::to_value(&vr)
        .map_err(|e| AttachMetaError::InternalError(format!("failed to serialize response: {e}")))
}

fn filter_by_prefix(
    items: &[crate::protocol::responses::ValidationError],
    prefix: &[String],
) -> Vec<crate::protocol::responses::ValidationError> {
    items
        .iter()
        .filter(|item| {
            // File-level errors (path: []) always pass through
            if item.path.is_empty() {
                return true;
            }
            // Check if item.path starts with prefix
            if item.path.len() < prefix.len() {
                return false;
            }
            item.path[..prefix.len()] == *prefix
        })
        .cloned()
        .collect()
}
