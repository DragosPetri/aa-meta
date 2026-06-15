use std::process::Stdio;

use anyhow::{bail, Result};

use crate::cli::{Cli, Command};
use crate::config::AppConfig;
use crate::discovery::run_discovery;

pub fn dispatch(cli: Cli, config: AppConfig) -> Result<()> {
    let tool_name = cli
        .tool
        .as_deref()
        .or(config.meta.default_tool.as_deref())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no tool specified — use --tool <name> or set default_tool in config"
            )
        })?;

    let tool = config.find_tool(tool_name).ok_or_else(|| {
        anyhow::anyhow!("tool '{tool_name}' not found in config")
    })?;

    let binary = &tool.binary;

    // discover is a special case: pass through directly to the tool's discovery command
    if matches!(cli.command, Command::Discover) {
        let mut extra = vec!["discovery"];
        if cli.json {
            extra.push("--json");
        }
        let status = std::process::Command::new(binary)
            .args(&extra)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| anyhow::anyhow!("failed to run '{binary}': {e}"))?;
        std::process::exit(status.code().unwrap_or(1));
    }

    let discovery = run_discovery(binary)?;

    let cmd_name = cli.command.name();
    let entry = discovery.commands.get(cmd_name).ok_or_else(|| {
        anyhow::anyhow!(
            "tool '{tool_name}' does not advertise a '{cmd_name}' command in its discovery output"
        )
    })?;

    if !entry.supported {
        bail!("tool '{tool_name}' does not support '{cmd_name}'");
    }

    let mut argv: Vec<&str> = entry.argv.iter().map(String::as_str).collect();

    if let Some(wf) = &cli.workfile {
        argv.push("--workfile");
        argv.push(wf.to_str().unwrap_or_default());
    }

    let user_args = cli.command.trailing_args();
    argv.extend(user_args.iter().map(String::as_str));

    if cli.json {
        argv.push("--json");
    }

    let status = std::process::Command::new(binary)
        .args(&argv)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| anyhow::anyhow!("failed to run '{binary}': {e}"))?;

    std::process::exit(status.code().unwrap_or(1));
}
