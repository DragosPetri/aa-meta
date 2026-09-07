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
            handle_completion(rest, cli.config.as_deref());
            return;
        }
        "__complete" => {
            // __complete -- <subcommand> [args...] <partial>
            let after_dash = if let Some(pos) = rest.iter().position(|a| a == "--") {
                &rest[pos + 1..]
            } else {
                rest
            };
            let (mut config, config_path) =
                config::load_config(cli.config.clone()).unwrap_or_default();
            complete::run_complete(after_dash, &mut config, &config_path, cli.tool.as_deref());
            return;
        }
        "init" => {
            let result = handle_init(rest, &cli);
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

    let (mut config, config_path) = match config::load_config(cli.config.clone()) {
        Ok(c) => c,
        Err(e) => exit_error(
            AttachMetaError::InternalError(format!("config error: {e}")),
            cli.json,
        ),
    };

    let tool_name = cli
        .tool
        .as_deref()
        .or(config.meta.default_tool.as_deref())
        .unwrap_or_else(|| {
            eprintln!(
                "attach-meta: no tool specified — use --tool <name> or set default_tool in config"
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

    // Validate input against effective schema
    let eff_schema = protocol::base_schema::effective_schema(cmd, mapping.args.as_ref());
    if let Err(e) = schema::validate_input(&eff_schema, &parsed.flags_json) {
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
    cli: &cli::Cli,
) -> std::result::Result<(CommandName, serde_json::Value), AttachMetaError> {
    if rest.is_empty() {
        return Err(AttachMetaError::InputError(
            "init requires <analog_attachable> argument".to_string(),
        ));
    }

    let binary = &rest[0];
    let no_interactive = rest.iter().any(|a| a == "--no-interactive");

    let (mut config, config_path) = config::load_config(cli.config.clone())
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

fn handle_completion(rest: &[String], config_path: Option<&std::path::Path>) {
    if rest.is_empty() {
        eprintln!("Usage: attach-meta completion <bash|zsh|fish>");
        std::process::exit(2);
    }

    let shell = &rest[0];
    match rest.get(1).map(|s| s.as_str()) {
        Some("--install") | None if rest.len() == 1 => {
            // Just print the script
            match complete::generate_completion_script(shell, config_path) {
                Ok(script) => print!("{script}"),
                Err(e) => {
                    eprintln!("attach-meta: {e}");
                    std::process::exit(2);
                }
            }
        }
        _ => match complete::generate_completion_script(shell, config_path) {
            Ok(script) => print!("{script}"),
            Err(e) => {
                eprintln!("attach-meta: {e}");
                std::process::exit(2);
            }
        },
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
