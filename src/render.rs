use crate::protocol::manifest::CommandName;
use crate::protocol::responses::*;

pub fn render_response(cmd: CommandName, response: &serde_json::Value, json_mode: bool) {
    if json_mode {
        println!(
            "{}",
            serde_json::to_string_pretty(response).unwrap_or_default()
        );
        return;
    }

    match cmd {
        CommandName::Add => render_add(response),
        CommandName::Read => render_read(response),
        CommandName::Delete => render_delete(response),
        CommandName::Validate => render_validate(response),
        CommandName::ListDevices => render_list_devices(response),
        CommandName::CreateWorkfile => render_create_workfile(response),
        CommandName::ToolConfigGet => render_tool_config(response),
        CommandName::ListIntelligence => render_list_intelligence(response),
        CommandName::Suggest => render_suggest(response),
        _ => render_common(response),
    }
}

fn render_common(v: &serde_json::Value) {
    if let Ok(r) = serde_json::from_value::<CommonResponse>(v.clone()) {
        let icon = match r.severity {
            Severity::Info => "✓",
            Severity::Warn => "⚠",
            Severity::Error => "✗",
        };
        eprintln!("{icon} {}", r.message);
    } else {
        println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
    }
}

fn render_add(v: &serde_json::Value) {
    if let Ok(r) = serde_json::from_value::<AddResponse>(v.clone()) {
        let path_str = r.path.join(" → ");
        eprintln!("{} (key: {}, path: {})", r.message, r.key, path_str);
    } else {
        render_common(v);
    }
}

fn render_read(v: &serde_json::Value) {
    if let Ok(r) = serde_json::from_value::<ReadResponse>(v.clone()) {
        match r {
            ReadResponse::Node(node) => render_node(&node, 0),
            ReadResponse::Property(prop) => {
                eprintln!("  {} = {}", prop.key, prop.value);
            }
        }
    } else {
        println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
    }
}

fn render_node(node: &Node, indent: usize) {
    let pad = "  ".repeat(indent);
    eprintln!("{pad}{}", node.key);
    if let Some(aliases) = &node.alias {
        if !aliases.is_empty() {
            eprintln!("{pad}  aliases: {}", aliases.join(", "));
        }
    }
    for prop in &node.properties {
        eprintln!("{pad}  {} = {}", prop.key, prop.value);
    }
    for child in &node.children {
        render_node(child, indent + 1);
    }
}

fn render_delete(v: &serde_json::Value) {
    if v.get("node_count").is_some() {
        if let Ok(r) = serde_json::from_value::<DeletePreview>(v.clone()) {
            eprintln!(
                "Delete preview: {} nodes, {} properties",
                r.node_count, r.property_count
            );
            for path in &r.paths {
                eprintln!("  {}", path.join(" → "));
            }
            eprintln!("Use --force to execute.");
            return;
        }
    }
    render_common(v);
}

fn render_validate(v: &serde_json::Value) {
    if let Ok(r) = serde_json::from_value::<ValidationResponse>(v.clone()) {
        if r.errors.is_empty() && r.warnings.is_empty() {
            eprintln!("✓ No issues found.");
        } else {
            for e in &r.errors {
                let path_str = if e.path.is_empty() {
                    "(file-level)".to_string()
                } else {
                    e.path.join(" → ")
                };
                eprintln!("✗ error [{}]: {}", path_str, e.message);
            }
            for w in &r.warnings {
                let path_str = if w.path.is_empty() {
                    "(file-level)".to_string()
                } else {
                    w.path.join(" → ")
                };
                eprintln!("⚠ warning [{}]: {}", path_str, w.message);
            }
        }
    } else {
        println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
    }
}

fn render_list_devices(v: &serde_json::Value) {
    if let Ok(r) = serde_json::from_value::<ListDevicesResponse>(v.clone()) {
        if r.devices.is_empty() {
            eprintln!("No devices found.");
        } else {
            for d in &r.devices {
                eprintln!("  {} ({})", d.key, d.tag);
            }
        }
    } else {
        render_common(v);
    }
}

fn render_create_workfile(v: &serde_json::Value) {
    if let Ok(r) = serde_json::from_value::<CreateWorkfileResponse>(v.clone()) {
        eprintln!("{} → {}", r.message, r.path);
    } else {
        render_common(v);
    }
}

fn render_tool_config(v: &serde_json::Value) {
    if let Ok(r) = serde_json::from_value::<ToolConfigResponse>(v.clone()) {
        for cfg in &r.configs {
            let required = if cfg.required { " (required)" } else { "" };
            let default_str = if cfg.default.is_null() {
                "none".to_string()
            } else {
                cfg.default.to_string()
            };
            eprintln!(
                "  {}{}: {} [default: {}]",
                cfg.field_name, required, cfg.description, default_str
            );
        }
    } else {
        render_common(v);
    }
}

fn render_list_intelligence(v: &serde_json::Value) {
    if let Ok(r) = serde_json::from_value::<ListIntelligenceResponse>(v.clone()) {
        if r.intelligence.is_empty() {
            eprintln!("No intelligence kinds available.");
        } else {
            for i in &r.intelligence {
                let arg_names: Vec<&str> = i.args.iter().map(|a| a.name.as_str()).collect();
                eprintln!("  {} (args: {})", i.kind, arg_names.join(", "));
            }
        }
    } else {
        render_common(v);
    }
}

fn render_suggest(v: &serde_json::Value) {
    if let Ok(r) = serde_json::from_value::<SuggestResponse>(v.clone()) {
        for s in &r.suggestions {
            if let Some(ds) = &s.display_string {
                eprintln!("  {} — {}", s.value, ds);
            } else {
                eprintln!("  {}", s.value);
            }
        }
    } else {
        render_common(v);
    }
}
