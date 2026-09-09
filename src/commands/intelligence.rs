use crate::commands::CommandContext;
use crate::error::AttachMetaError;
use crate::protocol::manifest::CommandName;
use crate::protocol::responses::ListIntelligenceResponse;
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
            let li_mapping = ctx
                .manifest
                .get_command(CommandName::ListIntelligence)
                .ok_or_else(|| {
                    AttachMetaError::ManifestError(
                        "command 'list-intelligence' not in manifest".to_string(),
                    )
                })?;

            let li_raw = transport::invoke(li_mapping, &[])?;
            let li_response: ListIntelligenceResponse =
                serde_json::from_value(li_raw).map_err(|e| {
                    AttachMetaError::InternalError(format!(
                        "failed to parse list-intelligence response: {e}"
                    ))
                })?;

            let kind = positionals.first().ok_or_else(|| {
                AttachMetaError::InputError("suggest requires a kind argument".to_string())
            })?;

            let advertised = li_response.intelligence.iter().any(|i| i.kind == *kind);

            if !advertised {
                return Err(AttachMetaError::InputError(format!(
                    "suggest kind '{kind}' is not advertised by list-intelligence"
                )));
            }

            transport::invoke(mapping, positionals)
        }
        _ => unreachable!(),
    }
}
