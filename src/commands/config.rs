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

    let mut extra_args = Vec::new();

    match cmd {
        CommandName::ToolConfigGet => {
            extra_args.extend(positionals.iter().cloned());
        }
        CommandName::ToolConfigSet => {
            if positionals.len() < 2 {
                return Err(AttachMetaError::InputError(
                    "tool-config-set requires <field> <value>".to_string(),
                ));
            }
            extra_args.extend(positionals.iter().cloned());
        }
        _ => unreachable!(),
    }

    transport::invoke(mapping, &extra_args)
}
