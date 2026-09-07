use crate::commands::CommandContext;
use crate::error::AttachMetaError;
use crate::protocol::manifest::CommandName;
use crate::transport;

pub fn run(
    cmd: CommandName,
    positionals: &[String],
    flags: &serde_json::Value,
    ctx: &CommandContext,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    let mapping = ctx.manifest.get_command(cmd).ok_or_else(|| {
        AttachMetaError::ManifestError(format!("command '{}' not in manifest", cmd))
    })?;

    let mut extra_args: Vec<String> = Vec::new();

    // Forward positionals (ValidIdentifier path)
    extra_args.extend(positionals.iter().cloned());

    // Forward flags
    if let Some(obj) = flags.as_object() {
        for (key, val) in obj {
            if val.is_boolean() {
                if val.as_bool() == Some(true) {
                    extra_args.push(format!("--{key}"));
                }
            } else if let Some(s) = val.as_str() {
                extra_args.push(format!("--{key}"));
                extra_args.push(s.to_string());
            }
        }
    }

    transport::invoke(mapping, &extra_args)
}
