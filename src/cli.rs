use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "attach-meta", version, about = "Meta-tool for analog attachable tools")]
pub struct Cli {
    #[arg(long, global = true, help = "Tool to use (overrides config default)")]
    pub tool: Option<String>,

    #[arg(long, global = true, help = "Workfile path forwarded to the tool")]
    pub workfile: Option<PathBuf>,

    #[arg(long, global = true, help = "Request machine-parseable output from the tool")]
    pub json: bool,

    #[arg(long, global = true, help = "Config file path (default: ~/.config/attach-meta/config.toml)")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Add a new node or primitive")]
    Create {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    #[command(about = "Read values of nodes or primitives")]
    Read {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    #[command(about = "Update primitive values")]
    Update {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    #[command(about = "Delete nodes or primitives")]
    Delete {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    #[command(about = "Validate workfile, node, or primitive")]
    Validate {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    #[command(about = "Generate an artifact from the workfile")]
    Generate {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    #[command(about = "Build from artifact")]
    Build {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    #[command(about = "Deploy built artifact to target")]
    Deploy {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    #[command(about = "Set a config value on the active tool")]
    Config {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    #[command(about = "Print discovery output for the active tool")]
    Discover,
}

impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Command::Create { .. }   => "create",
            Command::Read { .. }     => "read",
            Command::Update { .. }   => "update",
            Command::Delete { .. }   => "delete",
            Command::Validate { .. } => "validate",
            Command::Generate { .. } => "generate",
            Command::Build { .. }    => "build",
            Command::Deploy { .. }   => "deploy",
            Command::Config { .. }   => "config",
            Command::Discover        => "discover",
        }
    }

    pub fn trailing_args(&self) -> &[String] {
        match self {
            Command::Create { args }   => args,
            Command::Read { args }     => args,
            Command::Update { args }   => args,
            Command::Delete { args }   => args,
            Command::Validate { args } => args,
            Command::Generate { args } => args,
            Command::Build { args }    => args,
            Command::Deploy { args }   => args,
            Command::Config { args }   => args,
            Command::Discover          => &[],
        }
    }
}
