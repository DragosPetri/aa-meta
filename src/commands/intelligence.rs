use crate::commands::CommandContext;
use crate::error::AttachMetaError;
use crate::protocol::manifest::CommandName;
use crate::transport;

pub fn run(
    cmd: CommandName,
    positionals: &[String],
    _flags: &serde_json::Value,
    ctx: &CommandContext,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    let mapping = ctx.manifest.get_command(cmd).ok_or_else(|| {
        AttachMetaError::ManifestError(format!("command '{}' not in manifest", cmd))
    })?;

    match cmd {
        CommandName::ListIntelligence => transport::invoke(mapping, &[]),
        CommandName::Suggest => {
            // suggest <kind> [...args]
            transport::invoke(mapping, positionals)
        }
        _ => unreachable!(),
    }
}
