mod cli;
mod commands;
mod complete;
mod config;
mod dynargs;
mod error;
mod manifest_store;
mod prompt;
mod protocol;
mod render;
mod schema;
mod transport;

use error::{AttachMetaError, ErrorEnvelope};
use protocol::manifest::CommandName;

fn main() {
    let cli = cli::parse_phase1();

    let args = &cli.args;

    if args.is_empty() {
        eprintln!("attach-meta: no command provided — try --help");
        std::process::exit(2);
    }

    let command_str = &args[0];
    let rest = &args[1..];

    match command_str.as_str() {
        "completion" => {
            handle_completion(rest);
            return;
        }
        "__complete" => {
            // __complete -- <subcommand> [args...] <partial>
            let after_dash = if let Some(pos) = rest.iter().position(|a| a == "--") {
                &rest[pos + 1..]
            } else {
                rest
            };
            let (mut config, config_path) = config::load_config().unwrap_or_default();
            complete::run_complete(after_dash, &mut config, &config_path);
            return;
        }
        "init" => {
            let result = handle_init(rest);
            exit_with_result(result, cli.json);
            return;
        }
        _ => {}
    }

    // All other commands: load config + manifest, validate input, dispatch
    let cmd = match CommandName::from_str(command_str) {
        Some(c) => c,
        None => {
            eprintln!("attach-meta: unknown command '{command_str}'");
            std::process::exit(2);
        }
    };

    let (mut config, config_path) = match config::load_config() {
        Ok(c) => c,
        Err(e) => exit_error(
            AttachMetaError::InternalError(format!("config error: {e}")),
            cli.json,
        ),
    };

    let tool_name = config
        .meta
        .default_tool
        .as_deref()
        .unwrap_or_else(|| {
            eprintln!(
                "attach-meta: no tool specified — set default_tool in .attach-meta.toml"
            );
            std::process::exit(2);
        })
        .to_string();

    let tool = match config.find_tool(&tool_name) {
        Some(t) => t.clone(),
        None => {
            exit_error(
                AttachMetaError::ManifestError(format!("tool '{tool_name}' not found in config")),
                cli.json,
            );
        }
    };

    let verified = match manifest_store::load_verified(&tool, &mut config, &config_path) {
        Ok(v) => v,
        Err(e) => {
            exit_error(e, cli.json);
        }
    };

    let manifest = verified.manifest;

    // Check command is in manifest (move/rename have fallback workflows)
    let has_fallback = matches!(cmd, CommandName::Move | CommandName::Rename);
    let mapping = match manifest.get_command(cmd) {
        Some(m) => m.clone(),
        None if has_fallback => {
            // Use a synthetic mapping for arg parsing; the handler will use the fallback workflow
            protocol::manifest::CommandMapping {
                description: None,
                argv: vec![],
                args: None,
                timeout_ms: None,
                completions: None,
            }
        }
        None => {
            exit_error(
                AttachMetaError::ManifestError(format!(
                    "tool '{tool_name}' does not support '{}'",
                    cmd
                )),
                cli.json,
            );
        }
    };

    // Phase 2: parse args with manifest-augmented flags
    let parsed = match dynargs::parse_command_args(cmd, &mapping, rest) {
        Ok(p) => p,
        Err(e) => {
            exit_error(e, cli.json);
        }
    };

    // Validate input against effective schema.
    // x-positional properties (positional args) are injected into a temporary flags copy
    // so the schema's anyOf / required constraints can reference them normally.
    let eff_schema = protocol::base_schema::effective_schema(cmd, mapping.args.as_ref());
    let mut validation_flags = parsed.flags_json.clone();
    inject_x_positionals(cmd, &parsed.positionals, &mut validation_flags);
    if let Err(e) = schema::validate_input(&eff_schema, &validation_flags) {
        exit_error(e, cli.json);
    }

    // Dispatch
    let ctx = commands::CommandContext {
        manifest,
        tool_binary: tool.binary.unwrap_or_default(),
        json_output: cli.json,
        app_config: config,
        config_path,
    };

    let result = commands::dispatch(cmd, &parsed.positionals, &parsed.flags_json, &ctx);
    exit_with_result(result.map(|v| (cmd, v)), cli.json);
}

fn handle_init(
    rest: &[String],
) -> std::result::Result<(CommandName, serde_json::Value), AttachMetaError> {
    if rest.is_empty() {
        return Err(AttachMetaError::InputError(
            "init requires <analog_attachable> argument".to_string(),
        ));
    }

    let binary = &rest[0];
    let no_interactive = rest.iter().any(|a| a == "--no-interactive");

    let (mut config, config_path) = config::load_config()
        .map_err(|e| AttachMetaError::InternalError(format!("config error: {e}")))?;

    let prompter: Box<dyn prompt::Prompter> = if no_interactive || !prompt::is_interactive() {
        Box::new(prompt::ScriptedPrompter::new(vec![]))
    } else {
        Box::new(prompt::StdinPrompter)
    };

    let response = commands::init::run_init(
        binary,
        no_interactive,
        prompter.as_ref(),
        &mut config,
        &config_path,
    )?;

    // Dummy CommandName for rendering — init has its own response
    Ok((CommandName::ToolConfigGet, response))
}

fn handle_completion(rest: &[String]) {
    if rest.is_empty() {
        eprintln!("Usage: attach-meta completion <bash|zsh|fish>");
        std::process::exit(2);
    }

    let shell = &rest[0];
    match complete::generate_completion_script(shell) {
        Ok(script) => print!("{script}"),
        Err(e) => {
            eprintln!("attach-meta: {e}");
            std::process::exit(2);
        }
    }
}

fn exit_with_result(
    result: std::result::Result<(CommandName, serde_json::Value), AttachMetaError>,
    json_mode: bool,
) {
    match result {
        Ok((cmd, response)) => {
            // Check for ok:false in response (protocol error)
            if response.get("ok").and_then(|v| v.as_bool()) == Some(false) {
                render::render_response(cmd, &response, json_mode);
                std::process::exit(1);
            }
            render::render_response(cmd, &response, json_mode);
            std::process::exit(0);
        }
        Err(e) => exit_error(e, json_mode),
    }
}

fn exit_error(err: AttachMetaError, json_mode: bool) -> ! {
    let code = err.exit_code();
    if json_mode {
        let envelope = ErrorEnvelope::from_error(&err);
        println!(
            "{}",
            serde_json::to_string_pretty(&envelope).unwrap_or_default()
        );
    } else {
        eprintln!("attach-meta: {err}");
    }
    std::process::exit(code);
}

// Inject x-positional properties from the base schema into `flags` so that schema validation
// can evaluate anyOf / required constraints that reference them.  The original `flags_json`
// passed to dispatch is NOT modified — this only affects the temporary copy used for validation.
//
// Array x-positionals consume all remaining positionals (e.g. a variadic path).
// String x-positionals consume one positional each, in property-declaration order.
fn inject_x_positionals(
    cmd: CommandName,
    positionals: &[String],
    flags: &mut serde_json::Value,
) {
    let base = protocol::base_schema::base_schema(cmd);
    let props = match base.get("properties").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => return,
    };
    let mut pos_idx = 0;
    for (name, prop) in props {
        if prop.get("x-positional").and_then(|v| v.as_bool()) != Some(true) {
            continue;
        }
        if prop.get("type").and_then(|v| v.as_str()) == Some("array") {
            if let Some(obj) = flags.as_object_mut() {
                let values: Vec<_> = positionals[pos_idx..]
                    .iter()
                    .map(|s| serde_json::json!(s))
                    .collect();
                obj.insert(name.clone(), serde_json::Value::Array(values));
            }
            break;
        } else {
            if let (Some(val), Some(obj)) = (positionals.get(pos_idx), flags.as_object_mut()) {
                obj.insert(name.clone(), serde_json::json!(val));
            }
            pos_idx += 1;
        }
    }
}
