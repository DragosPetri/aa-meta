use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{Result, bail};

use crate::cli::Command;
use crate::config::AppConfig;
use crate::discovery::run_discovery;

/// Call the tool's `complete <subcommand> <partial>` and print results to stdout.
/// Fails completely silently unless `verbose` is set, in which case a trace goes to stderr.
pub fn run_complete(
    subcommand: &str,
    partial: &str,
    tool_override: Option<String>,
    config: AppConfig,
    verbose: bool,
) {
    macro_rules! trace {
        ($($arg:tt)*) => {
            if verbose { eprintln!("[attach-meta complete] {}", format!($($arg)*)); }
        };
    }

    let Some(tool_name) = tool_override
        .as_deref()
        .or(config.meta.default_tool.as_deref())
    else {
        trace!("no tool configured — set default_tool in config or pass --tool");
        return;
    };
    trace!("resolved tool: {tool_name}");

    let Some(tool) = config.find_tool(tool_name) else {
        trace!(
            "tool '{tool_name}' not found in config (tools listed: {})",
            config
                .tools
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        return;
    };
    trace!("binary: {}", tool.binary);

    let discovery = match run_discovery(&tool.binary) {
        Ok(d) => d,
        Err(e) => {
            trace!("discovery failed: {e}");
            return;
        }
    };
    trace!(
        "discovery ok — commands advertised: {}",
        discovery
            .commands
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    );

    match discovery.commands.get("complete") {
        Some(e) if e.supported => {
            trace!("'complete' command supported");
        }
        Some(_) => {
            trace!("'complete' is listed in discovery but marked supported=false");
            return;
        }
        None => {
            trace!(
                "'complete' not found in discovery output — tool does not implement completions"
            );
            return;
        }
    }

    let argv = ["complete", subcommand, partial];
    trace!("calling: {} {}", tool.binary, argv.join(" "));

    let output = match std::process::Command::new(&tool.binary).args(argv).output() {
        Ok(o) => o,
        Err(e) => {
            trace!("failed to run binary: {e}");
            return;
        }
    };

    trace!("exit status: {}", output.status);
    if !output.stderr.is_empty() {
        trace!(
            "tool stderr: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        trace!("{} candidates returned", stdout.lines().count());
        if verbose && stdout.is_empty() {
            eprintln!("[attach-meta complete] stdout is empty — tool printed nothing");
        } else if verbose {
            for (i, line) in stdout.lines().enumerate() {
                eprintln!("[attach-meta complete]   [{i}] {:?}", line);
            }
        }
        print!("{stdout}");
    } else {
        trace!(
            "tool exited non-zero — stdout: {:?}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

pub fn dispatch(
    command: Command,
    tool_override: Option<String>,
    workfile: Option<PathBuf>,
    json: bool,
    config: AppConfig,
) -> Result<()> {
    let tool_name = tool_override
        .as_deref()
        .or(config.meta.default_tool.as_deref())
        .ok_or_else(|| {
            anyhow::anyhow!("no tool specified — use --tool <name> or set default_tool in config")
        })?;

    let tool = config
        .find_tool(tool_name)
        .ok_or_else(|| anyhow::anyhow!("tool '{tool_name}' not found in config"))?;

    let binary = &tool.binary;

    // discover is a special case: pass through directly to the tool's discovery command
    if matches!(command, Command::Discover) {
        let mut extra = Vec::new();
        if json {
            extra.push("--json");
        }
        extra.push("discovery");
        let status = std::process::Command::new(binary)
            .args(&extra)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| anyhow::anyhow!("failed to run '{binary}': {e}"))?;
        std::process::exit(status.code().unwrap_or(1));
    }

    let discovery = run_discovery(binary)?;

    let key = command.key();
    let cmd_name = key.spec().key;
    let entry = discovery.commands.get(cmd_name).ok_or_else(|| {
        anyhow::anyhow!(
            "tool '{tool_name}' does not advertise a '{cmd_name}' command in its discovery output"
        )
    })?;

    if !entry.supported {
        bail!("tool '{tool_name}' does not support '{cmd_name}'");
    }

    let mut argv: Vec<&str> = Vec::new();

    if json {
        argv.push("--json");
    }

    argv.extend(entry.argv.iter().map(String::as_str));

    if let Some(wf) = &workfile {
        argv.push("--workfile");
        argv.push(wf.to_str().unwrap_or_default());
    }

    argv.extend(command.trailing_args().iter().map(String::as_str));

    let status = std::process::Command::new(binary)
        .args(&argv)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| anyhow::anyhow!("failed to run '{binary}': {e}"))?;

    std::process::exit(status.code().unwrap_or(1));
}
