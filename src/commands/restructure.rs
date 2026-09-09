use crate::commands::CommandContext;
use crate::error::AttachMetaError;
use crate::protocol::manifest::CommandName;
use crate::protocol::responses::{Node, ReadResponse};
use crate::transport;

// --with takes a raw string. Unwrap JSON strings to their inner value;
// for all other types (numbers, bools, arrays) use JSON serialization.
fn raw_value(v: &serde_json::Value) -> String {
    if let Some(s) = v.as_str() {
        s.to_string()
    } else {
        v.to_string()
    }
}

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

fn append_segment(parent: &[String], seg: &str) -> Vec<String> {
    let mut v = parent.to_vec();
    v.push(seg.to_string());
    v
}

fn push_array_flag(args: &mut Vec<String>, flag: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    args.push(format!("--{flag}"));
    args.extend(values.iter().cloned());
}

fn run_move(
    positionals: &[String],
    flags: &serde_json::Value,
    ctx: &CommandContext,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    let destination = get_array_flag(flags, "to");

    // Native move if available
    if let Some(mapping) = ctx.manifest.get_command(CommandName::Move) {
        let mut args = positionals.to_vec();
        push_array_flag(&mut args, "to", &destination);
        return transport::invoke(mapping, &args);
    }

    if destination.is_empty() {
        return Err(AttachMetaError::InputError(
            "--to is required for move".to_string(),
        ));
    }

    // Fallback: read → add → update → delete --force
    let read_mapping = ctx
        .manifest
        .get_command(CommandName::Read)
        .ok_or_else(|| AttachMetaError::ManifestError("'read' not in manifest".to_string()))?;
    let read_response = transport::invoke(read_mapping, positionals).map_err(|e| {
        AttachMetaError::TransportError(format!("move fallback step 1 (read) failed: {e}"))
    })?;

    let node: Node = match serde_json::from_value::<ReadResponse>(read_response) {
        Ok(ReadResponse::Node(n)) => n,
        Ok(ReadResponse::Property(_)) => {
            return Err(AttachMetaError::InputError(
                "cannot move a property — only nodes are movable".to_string(),
            ));
        }
        Err(e) => {
            return Err(AttachMetaError::TransportError(format!(
                "move fallback: failed to parse read response: {e}"
            )));
        }
    };

    // Steps 2-3: depth-first add + update under new parent
    let add_mapping = ctx
        .manifest
        .get_command(CommandName::Add)
        .ok_or_else(|| AttachMetaError::ManifestError("'add' not in manifest".to_string()))?;
    let update_mapping = ctx
        .manifest
        .get_command(CommandName::Update)
        .ok_or_else(|| AttachMetaError::ManifestError("'update' not in manifest".to_string()))?;

    recreate_subtree(
        &node,
        &destination,
        "move fallback",
        add_mapping,
        update_mapping,
    )?;

    // Step 4: delete old node with --force
    let delete_mapping = ctx
        .manifest
        .get_command(CommandName::Delete)
        .ok_or_else(|| AttachMetaError::ManifestError("'delete' not in manifest".to_string()))?;

    let mut del_args = positionals.to_vec();
    del_args.push("--force".to_string());

    transport::invoke(delete_mapping, &del_args).map_err(|e| {
        AttachMetaError::TransportError(format!(
            "move fallback step 4 (delete --force) failed: {e}"
        ))
    })
}

fn run_rename(
    positionals: &[String],
    flags: &serde_json::Value,
    ctx: &CommandContext,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    // Native rename if available
    if let Some(mapping) = ctx.manifest.get_command(CommandName::Rename) {
        let mut args = positionals.to_vec();
        if let Some(to) = flags.get("to").and_then(|v| v.as_str()) {
            args.push("--to".to_string());
            args.push(to.to_string());
        }
        return transport::invoke(mapping, &args);
    }

    let to = flags
        .get("to")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AttachMetaError::InputError("--to is required for rename".to_string()))?;

    // Fallback: determine if node or property
    let read_mapping = ctx
        .manifest
        .get_command(CommandName::Read)
        .ok_or_else(|| AttachMetaError::ManifestError("'read' not in manifest".to_string()))?;
    let read_response = transport::invoke(read_mapping, positionals).map_err(|e| {
        AttachMetaError::TransportError(format!("rename fallback step 1 (read) failed: {e}"))
    })?;

    match serde_json::from_value::<ReadResponse>(read_response) {
        Ok(ReadResponse::Property(prop)) => {
            // Property rename: read → upsert → delete
            let update_mapping =
                ctx.manifest
                    .get_command(CommandName::Update)
                    .ok_or_else(|| {
                        AttachMetaError::ManifestError("'update' not in manifest".to_string())
                    })?;

            // Build path to new property: replace last element with new name
            let mut new_path = positionals[..positionals.len().saturating_sub(1)].to_vec();
            new_path.push(to.to_string());
            let mut update_args = new_path;
            update_args.push("--with".to_string());
            update_args.push(raw_value(&prop.value));

            transport::invoke(update_mapping, &update_args).map_err(|e| {
                AttachMetaError::TransportError(format!(
                    "rename fallback step 2 (upsert) failed: {e}"
                ))
            })?;

            // Delete old property
            let delete_mapping =
                ctx.manifest
                    .get_command(CommandName::Delete)
                    .ok_or_else(|| {
                        AttachMetaError::ManifestError("'delete' not in manifest".to_string())
                    })?;
            transport::invoke(delete_mapping, positionals).map_err(|e| {
                AttachMetaError::TransportError(format!(
                    "rename fallback step 3 (delete) failed: {e}"
                ))
            })
        }
        Ok(ReadResponse::Node(node)) => {
            // Node rename: full subtree recreation under new name, then delete
            let add_mapping = ctx.manifest.get_command(CommandName::Add).ok_or_else(|| {
                AttachMetaError::ManifestError("'add' not in manifest".to_string())
            })?;
            let update_mapping =
                ctx.manifest
                    .get_command(CommandName::Update)
                    .ok_or_else(|| {
                        AttachMetaError::ManifestError("'update' not in manifest".to_string())
                    })?;

            // Parent path = positionals minus the last element (the node being renamed)
            let parent_path = &positionals[..positionals.len().saturating_sub(1)];

            // Add new node with new name under same parent
            let mut add_args = vec!["--name".to_string(), to.to_string()];
            push_array_flag(&mut add_args, "to", parent_path);
            transport::invoke(add_mapping, &add_args).map_err(|e| {
                AttachMetaError::TransportError(format!("rename fallback step 2 (add) failed: {e}"))
            })?;

            // Full path to the new node
            let new_node_path = append_segment(parent_path, to);

            // Update properties on new node: path = [...parent, to, prop.key]
            let mut update_args = new_node_path.clone();
            for prop in &node.properties {
                update_args.truncate(new_node_path.len());
                update_args.push(prop.key.clone());
                update_args.push("--with".to_string());
                update_args.push(raw_value(&prop.value));
                transport::invoke(update_mapping, &update_args).map_err(|e| {
                    AttachMetaError::TransportError(format!(
                        "rename fallback step 3 (update) failed: {e}"
                    ))
                })?;
            }

            // Recreate children under new node
            for child in &node.children {
                recreate_subtree(
                    child,
                    &new_node_path,
                    "rename fallback",
                    add_mapping,
                    update_mapping,
                )?;
            }

            // Delete old node
            let delete_mapping =
                ctx.manifest
                    .get_command(CommandName::Delete)
                    .ok_or_else(|| {
                        AttachMetaError::ManifestError("'delete' not in manifest".to_string())
                    })?;
            let mut del_args = positionals.to_vec();
            del_args.push("--force".to_string());
            transport::invoke(delete_mapping, &del_args).map_err(|e| {
                AttachMetaError::TransportError(format!(
                    "rename fallback step 5 (delete --force) failed: {e}"
                ))
            })
        }
        Err(e) => Err(AttachMetaError::TransportError(format!(
            "rename fallback: failed to parse read response: {e}"
        ))),
    }
}

fn recreate_subtree(
    node: &Node,
    parent_path: &[String],
    context: &str,
    add_mapping: &crate::protocol::manifest::CommandMapping,
    update_mapping: &crate::protocol::manifest::CommandMapping,
) -> std::result::Result<(), AttachMetaError> {
    let mut add_args = vec!["--key".to_string(), node.key.clone()];
    push_array_flag(&mut add_args, "to", parent_path);
    transport::invoke(add_mapping, &add_args).map_err(|e| {
        AttachMetaError::TransportError(format!("{context} (add '{}') failed: {e}", node.key))
    })?;

    let node_path = append_segment(parent_path, &node.key);

    let mut update_args = node_path.clone();
    for prop in &node.properties {
        update_args.truncate(node_path.len());
        update_args.push(prop.key.clone());
        update_args.push("--with".to_string());
        update_args.push(raw_value(&prop.value));
        transport::invoke(update_mapping, &update_args).map_err(|e| {
            AttachMetaError::TransportError(format!(
                "{context} (update '{}') failed: {e}",
                prop.key
            ))
        })?;
    }

    for child in &node.children {
        recreate_subtree(child, &node_path, context, add_mapping, update_mapping)?;
    }

    Ok(())
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
