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
    match cmd {
        CommandName::Move => run_move(positionals, flags, ctx),
        CommandName::Rename => run_rename(positionals, flags, ctx),
        CommandName::Alias => run_alias(positionals, flags, ctx),
        _ => unreachable!(),
    }
}

fn push_array_flag(args: &mut Vec<String>, flag: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    args.push(format!("--{flag}"));
    args.extend(values.iter().cloned());
}

fn get_array_flag(flags: &serde_json::Value, key: &str) -> Vec<String> {
    flags
        .get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn run_move(
    positionals: &[String],
    flags: &serde_json::Value,
    ctx: &CommandContext,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    let mapping = ctx
        .manifest
        .get_command(CommandName::Move)
        .ok_or_else(|| {
            AttachMetaError::ManifestError(
                "command 'move' not in manifest — tool does not support move".to_string(),
            )
        })?;

    let mut args = positionals.to_vec();
    let destination = get_array_flag(flags, "to");
    push_array_flag(&mut args, "to", &destination);

    transport::invoke(mapping, &args)
}

fn run_rename(
    positionals: &[String],
    flags: &serde_json::Value,
    ctx: &CommandContext,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    let mapping = ctx
        .manifest
        .get_command(CommandName::Rename)
        .ok_or_else(|| {
            AttachMetaError::ManifestError(
                "command 'rename' not in manifest — tool does not support rename".to_string(),
            )
        })?;

    let mut args = positionals.to_vec();
    if let Some(to) = flags.get("to").and_then(|v| v.as_str()) {
        args.push("--to".to_string());
        args.push(to.to_string());
    }

    transport::invoke(mapping, &args)
}

fn run_alias(
    positionals: &[String],
    flags: &serde_json::Value,
    ctx: &CommandContext,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    let mapping = ctx
        .manifest
        .get_command(CommandName::Alias)
        .ok_or_else(|| {
            AttachMetaError::ManifestError(
                "command 'alias' not in manifest — tool does not support aliases".to_string(),
            )
        })?;

    let mut args = positionals.to_vec();

    if let Some(with) = flags.get("with").and_then(|v| v.as_str()) {
        args.push("--with".to_string());
        args.push(with.to_string());
    } else if let Some(remove) = flags.get("remove").and_then(|v| v.as_str()) {
        args.push("--remove".to_string());
        args.push(remove.to_string());
    }

    transport::invoke(mapping, &args)
}
