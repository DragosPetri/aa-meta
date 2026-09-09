use crate::commands::CommandContext;
use crate::error::AttachMetaError;
use crate::protocol::manifest::CommandName;
use crate::protocol::responses::{Node, ReadResponse};
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

fn run_move(
    positionals: &[String],
    flags: &serde_json::Value,
    ctx: &CommandContext,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    // Native move if available
    if let Some(mapping) = ctx.manifest.get_command(CommandName::Move) {
        let mut args = positionals.to_vec();
        if let Some(to) = flags.get("to").and_then(|v| v.as_str()) {
            args.push("--to".to_string());
            args.push(to.to_string());
        }
        return transport::invoke(mapping, &args);
    }

    // Fallback: read → add → update → delete --force
    let to = flags
        .get("to")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AttachMetaError::InputError("--to is required for move".to_string()))?;

    // Step 1: read the source node
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

    fn recreate_subtree(
        node: &Node,
        parent_path: &str,
        add_mapping: &crate::protocol::manifest::CommandMapping,
        update_mapping: &crate::protocol::manifest::CommandMapping,
    ) -> std::result::Result<(), AttachMetaError> {
        // Add node under new parent
        let add_args = vec![
            "--key".to_string(),
            node.key.clone(),
            "--parent".to_string(),
            parent_path.to_string(),
        ];
        transport::invoke(add_mapping, &add_args).map_err(|e| {
            AttachMetaError::TransportError(format!(
                "move fallback step 2 (add '{}') failed: {e}",
                node.key
            ))
        })?;

        // Update properties
        for prop in &node.properties {
            let update_args = vec![
                node.key.clone(),
                prop.key.clone(),
                "--with".to_string(),
                prop.value.to_string(),
            ];
            transport::invoke(update_mapping, &update_args).map_err(|e| {
                AttachMetaError::TransportError(format!(
                    "move fallback step 3 (update '{}') failed: {e}",
                    prop.key
                ))
            })?;
        }

        // Recurse into children
        for child in &node.children {
            recreate_subtree(child, &node.key, add_mapping, update_mapping)?;
        }

        Ok(())
    }

    recreate_subtree(&node, to, add_mapping, update_mapping)?;

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
            update_args.push(prop.value.to_string());

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

            // Determine parent from positionals (all but last)
            let parent = if positionals.len() > 1 {
                positionals[..positionals.len() - 1].join(" ")
            } else {
                String::new()
            };

            // Add new node with new name under same parent
            let mut add_args = vec!["--name".to_string(), to.to_string()];
            if !parent.is_empty() {
                add_args.push("--parent".to_string());
                add_args.push(parent);
            }
            transport::invoke(add_mapping, &add_args).map_err(|e| {
                AttachMetaError::TransportError(format!("rename fallback step 2 (add) failed: {e}"))
            })?;

            // Update properties on new node
            for prop in &node.properties {
                let mut update_args = vec![to.to_string(), prop.key.clone()];
                update_args.push("--with".to_string());
                update_args.push(prop.value.to_string());
                transport::invoke(update_mapping, &update_args).map_err(|e| {
                    AttachMetaError::TransportError(format!(
                        "rename fallback step 3 (update) failed: {e}"
                    ))
                })?;
            }

            // Recreate children under new node
            for child in &node.children {
                recreate_child(&child, to, add_mapping, update_mapping)?;
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

fn recreate_child(
    node: &Node,
    parent: &str,
    add_mapping: &crate::protocol::manifest::CommandMapping,
    update_mapping: &crate::protocol::manifest::CommandMapping,
) -> std::result::Result<(), AttachMetaError> {
    let add_args = vec![
        "--key".to_string(),
        node.key.clone(),
        "--parent".to_string(),
        parent.to_string(),
    ];
    transport::invoke(add_mapping, &add_args).map_err(|e| {
        AttachMetaError::TransportError(format!(
            "rename fallback (add child '{}') failed: {e}",
            node.key
        ))
    })?;

    for prop in &node.properties {
        let update_args = vec![
            node.key.clone(),
            prop.key.clone(),
            "--with".to_string(),
            prop.value.to_string(),
        ];
        transport::invoke(update_mapping, &update_args).map_err(|e| {
            AttachMetaError::TransportError(format!(
                "rename fallback (update '{}') failed: {e}",
                prop.key
            ))
        })?;
    }

    for child in &node.children {
        recreate_child(child, &node.key, add_mapping, update_mapping)?;
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
