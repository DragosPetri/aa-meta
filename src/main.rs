mod cli;
mod config;
mod discovery;
mod router;
mod schema;

use std::path::PathBuf;

use clap::{CommandFactory, Parser};
use clap_complete::{Shell, generate};
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();

    if let Some(shell) = cli.setup_completions {
        if let Err(e) = setup_completions(shell, cli.tool.clone(), cli.config.clone()) {
            eprintln!("attach-meta: {e}");
            std::process::exit(1);
        }
        return;
    }

    let command = cli.command.unwrap_or_else(|| {
        eprintln!("attach-meta: no command provided — try --help");
        std::process::exit(1);
    });

    if let Command::Completions { shell } = command {
        generate(
            shell,
            &mut Cli::command(),
            "attach-meta",
            &mut std::io::stdout(),
        );
        return;
    }

    if let Command::Schema = command {
        print_schema(cli.json);
        return;
    }

    if let Command::Complete {
        subcommand,
        partial,
        tokens,
    } = command
    {
        let config = config::load_config(cli.config.clone()).unwrap_or_default();
        router::run_complete(
            &subcommand,
            partial.as_deref().unwrap_or(""),
            &tokens,
            cli.tool,
            config,
            cli.verbose,
        );
        return;
    }

    let config = config::load_config(cli.config.clone()).unwrap_or_else(|e| {
        eprintln!("attach-meta: config error: {e}");
        std::process::exit(1);
    });
    if let Err(e) = router::dispatch(command, cli.tool, cli.workfile, cli.json, config) {
        eprintln!("attach-meta: {e}");
        std::process::exit(1);
    }
}

fn print_schema(json: bool) {
    use schema::DiscoveryKey;
    let version = env!("CARGO_PKG_VERSION");
    if json {
        // Skeleton discovery JSON a tool author can copy and fill in.
        println!("{{");
        println!("  \"protocol_version\": \"{version}\",");
        println!("  \"tool_name\": \"<your-tool-name>\",");
        println!("  \"tool_version\": \"<your-tool-version>\",");
        println!("  \"commands\": {{");
        let all = DiscoveryKey::ALL;
        for (i, dk) in all.iter().enumerate() {
            let spec = dk.spec();
            let argv_json: String = spec.argv.iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(", ");
            let comma = if i < all.len() - 1 { "," } else { "" };
            println!(
                "    \"{}\": {{ \"argv\": [{argv_json}], \"supported\": true }}{comma}",
                spec.key
            );
        }
        println!("  }}");
        println!("}}");
    } else {
        println!("attach-meta protocol {version} — expected discovery commands\n");
        let key_width = DiscoveryKey::ALL.iter().map(|dk| dk.spec().key.len()).max().unwrap_or(0);
        for dk in DiscoveryKey::ALL {
            let spec = dk.spec();
            let argv = spec.argv.join(" ");
            println!("  {:<key_width$}  argv: [{argv}]", spec.key);
            println!("  {:<key_width$}  {}", "", spec.description);
            println!();
        }
    }
}

fn setup_completions(
    shell: Shell,
    tool: Option<String>,
    config_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    let path = completion_path(shell)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = match shell {
        Shell::Zsh => {
            let config = config::load_config(config_path).unwrap_or_default();
            let tool_name = tool
                .as_deref()
                .or(config.meta.default_tool.as_deref())
                .unwrap_or("(none)");
            generate_zsh_script(tool_name)
        }
        _ => {
            let mut buf = Vec::new();
            generate(shell, &mut Cli::command(), "attach-meta", &mut buf);
            String::from_utf8(buf)?
        }
    };

    std::fs::write(&path, content)?;
    println!("Installed {} completions to {}", shell, path.display());
    maybe_print_hint(shell, &path);

    Ok(())
}

fn generate_zsh_script(tool_name: &str) -> String {
    // The subcommands that accept tool arguments and should get dynamic completions.
    let dynamic = [
        "update", "validate", "generate", "build", "deploy", "config", "init", "list-devices",
    ];
    let dynamic_cases: String = dynamic.iter().map(|cmd| {
        format!(
            "            ({cmd})\n                local -a _pos_tokens tool_completions\n                _pos_tokens=(${{(M)words[2,$(($CURRENT-1))]:#^-*}})\n                tool_completions=(${{(f)\"$(attach-meta _complete {cmd} \"${{words[$CURRENT]}}\" \"${{_pos_tokens[@]}}\" 2>/dev/null)\"}})\n                compadd -a tool_completions\n            ;;\n",
        )
    }).collect();
    // create/delete have fixed subcommands (node/property), each with dynamic tool completions.
    let create_case = concat!(
        "            (create)\n",
        "                if [[ $CURRENT -eq 2 ]]; then\n",
        "                    local -a create_subs\n",
        "                    create_subs=(node property workfile)\n",
        "                    _describe 'create subcommand' create_subs\n",
        "                else\n",
        "                    local -a _pos_tokens\n",
        "                    _pos_tokens=(${(M)words[3,$(($CURRENT-1))]:#^-*})\n",
        "                    case $words[2] in\n",
        "                        (node)\n",
        "                            local -a tool_completions\n",
        "                            tool_completions=(${(f)\"$(attach-meta _complete create_node \"${words[$CURRENT]}\" \"${_pos_tokens[@]}\" 2>/dev/null)\"})\n",
        "                            compadd -a tool_completions\n",
        "                        ;;\n",
        "                        (property)\n",
        "                            local -a tool_completions\n",
        "                            tool_completions=(${(f)\"$(attach-meta _complete create_property \"${words[$CURRENT]}\" \"${_pos_tokens[@]}\" 2>/dev/null)\"})\n",
        "                            compadd -a tool_completions\n",
        "                        ;;\n",
        "                        (workfile)\n",
        "                            local -a tool_completions\n",
        "                            tool_completions=(${(f)\"$(attach-meta _complete create_workfile \"${words[$CURRENT]}\" \"${_pos_tokens[@]}\" 2>/dev/null)\"})\n",
        "                            compadd -a tool_completions\n",
        "                        ;;\n",
        "                    esac\n",
        "                fi\n",
        "            ;;\n",
    );
    let read_case = concat!(
        "            (read)\n",
        "                if [[ $CURRENT -eq 2 ]]; then\n",
        "                    local -a read_subs\n",
        "                    read_subs=(node property)\n",
        "                    _describe 'read subcommand' read_subs\n",
        "                else\n",
        "                    local -a _pos_tokens\n",
        "                    _pos_tokens=(${(M)words[3,$(($CURRENT-1))]:#^-*})\n",
        "                    case $words[2] in\n",
        "                        (node)\n",
        "                            local -a tool_completions\n",
        "                            tool_completions=(${(f)\"$(attach-meta _complete read_node \"${words[$CURRENT]}\" \"${_pos_tokens[@]}\" 2>/dev/null)\"})\n",
        "                            compadd -a tool_completions\n",
        "                        ;;\n",
        "                        (property)\n",
        "                            local -a tool_completions\n",
        "                            tool_completions=(${(f)\"$(attach-meta _complete read_property \"${words[$CURRENT]}\" \"${_pos_tokens[@]}\" 2>/dev/null)\"})\n",
        "                            compadd -a tool_completions\n",
        "                        ;;\n",
        "                    esac\n",
        "                fi\n",
        "            ;;\n",
    );
    let delete_case = concat!(
        "            (delete)\n",
        "                if [[ $CURRENT -eq 2 ]]; then\n",
        "                    local -a delete_subs\n",
        "                    delete_subs=(node property)\n",
        "                    _describe 'delete subcommand' delete_subs\n",
        "                else\n",
        "                    local -a _pos_tokens\n",
        "                    _pos_tokens=(${(M)words[3,$(($CURRENT-1))]:#^-*})\n",
        "                    case $words[2] in\n",
        "                        (node)\n",
        "                            local -a tool_completions\n",
        "                            tool_completions=(${(f)\"$(attach-meta _complete delete_node \"${words[$CURRENT]}\" \"${_pos_tokens[@]}\" 2>/dev/null)\"})\n",
        "                            compadd -a tool_completions\n",
        "                        ;;\n",
        "                        (property)\n",
        "                            local -a tool_completions\n",
        "                            tool_completions=(${(f)\"$(attach-meta _complete delete_property \"${words[$CURRENT]}\" \"${_pos_tokens[@]}\" 2>/dev/null)\"})\n",
        "                            compadd -a tool_completions\n",
        "                        ;;\n",
        "                    esac\n",
        "                fi\n",
        "            ;;\n",
    );

    format!(
        r#"#compdef attach-meta
# Generated by attach-meta --setup-completions zsh
# Active tool: {tool_name}

_attach-meta() {{
    local state

    _arguments \
        '--tool[Tool to use]:tool:' \
        '--workfile[Workfile path]:file:_files' \
        '--json[Machine-parseable output]' \
        '--config[Config file]:file:_files' \
        '--setup-completions[Install completions]:shell:(bash zsh fish elvish powershell)' \
        '(-): :->command' \
        '(-)*:: :->args'

    case $state in
        command)
            local -a commands
            commands=(
                'create:Add a new node or property'
                'read:Read values of a node or property'
                'update:Update primitive values'
                'delete:Delete a node or property'
                'validate:Validate workfile, node, or primitive'
                'generate:Generate an artifact from the workfile'
                'build:Build from artifact'
                'deploy:Deploy built artifact to target'
                'config:Set a config value on the active tool'
                'init:Initialize a new workfile or project'
                'list-devices:List available devices'
                'discover:Print discovery output for the active tool'
                'schema:Print the protocol command table'
                'completions:Print shell completion script'
            )
            _describe 'command' commands
        ;;
        args)
            case $words[1] in
{dynamic_cases}{create_case}{read_case}{delete_case}            (completions)
                local -a shells
                shells=(bash zsh fish elvish powershell)
                _describe 'shell' shells
            ;;
            esac
        ;;
    esac
}}

_attach-meta "$@"
"#
    )
}

fn completion_path(shell: Shell) -> anyhow::Result<PathBuf> {
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;

    let path = match shell {
        Shell::Bash => home.join(".local/share/bash-completion/completions/attach-meta"),
        Shell::Zsh => home.join(".zsh/completions/_attach-meta"),
        Shell::Fish => dirs::config_dir()
            .unwrap_or_else(|| home.join(".config"))
            .join("fish/completions/attach-meta.fish"),
        Shell::Elvish => dirs::config_dir()
            .unwrap_or_else(|| home.join(".config"))
            .join("elvish/lib/attach-meta.elv"),
        Shell::PowerShell => dirs::document_dir()
            .unwrap_or_else(|| home.join("Documents"))
            .join("PowerShell/Completions/attach-meta.ps1"),
        _ => anyhow::bail!("unsupported shell: {shell}"),
    };

    Ok(path)
}

fn maybe_print_hint(shell: Shell, path: &PathBuf) {
    match shell {
        Shell::Zsh => {
            let dir = path.parent().unwrap().display().to_string();
            println!("Hint: ensure {dir} is in your fpath before compinit, e.g.:");
            println!("  fpath=({dir} $fpath)");
            println!("  autoload -Uz compinit && compinit");
        }
        Shell::Bash => {
            println!("Hint: restart your shell or run: source {}", path.display());
        }
        _ => {}
    }
}
