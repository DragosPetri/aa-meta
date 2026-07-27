use std::path::PathBuf;

use clap::{Parser, Subcommand};
pub use clap_complete::Shell;

// TODO: name tbd
#[derive(Debug, Parser)]
#[command(
    name = "attach-meta",
    version,
    about = "Meta-tool for analog attachable tools"
)]
pub struct Cli {
    #[arg(long, global = true, help = "Tool to use (overrides config default)")]
    pub tool: Option<String>,

    // TODO: not conviced only one workfile
    #[arg(long, global = true, help = "Workfile path forwarded to the tool")]
    pub workfile: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        help = "Request machine-parseable output from the tool"
    )]
    pub json: bool,

    // TODO: not conviced only file possible, maybe also config folder or workspace
    #[arg(
        long,
        global = true,
        help = "Config file path (default: ~/.config/attach-meta/config.toml)"
    )]
    pub config: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        value_name = "SHELL",
        help = "Install shell completions to the standard location and exit"
    )]
    pub setup_completions: Option<Shell>,

    #[arg(long, global = true, help = "Print verbose trace to stderr")]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum CreateSubcommand {
    #[command(about = "Add a new node")]
    Node {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    #[command(about = "Add a new property")]
    Property {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
}

impl CreateSubcommand {
    pub fn discovery_name(&self) -> &'static str {
        match self {
            CreateSubcommand::Node { .. } => "create_node",
            CreateSubcommand::Property { .. } => "create_property",
        }
    }

    pub fn trailing_args(&self) -> &[String] {
        match self {
            CreateSubcommand::Node { args } | CreateSubcommand::Property { args } => args,
        }
    }
}

// TODO: incomplete interface for sure
#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Add a new node or property")]
    Create {
        #[command(subcommand)]
        subcommand: CreateSubcommand,
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
    #[command(about = "Print shell completion script for the given shell")]
    Completions { shell: Shell },
    #[command(name = "_complete", hide = true)]
    Complete {
        subcommand: String,
        partial: Option<String>,
    },
}

impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Command::Create { subcommand } => subcommand.discovery_name(),
            Command::Read { .. } => "read",
            Command::Update { .. } => "update",
            Command::Delete { .. } => "delete",
            Command::Validate { .. } => "validate",
            Command::Generate { .. } => "generate",
            Command::Build { .. } => "build",
            Command::Deploy { .. } => "deploy",
            Command::Config { .. } => "config",
            Command::Discover => "discover",
            Command::Completions { .. } => "completions",
            Command::Complete { .. } => "_complete",
        }
    }

    pub fn trailing_args(&self) -> &[String] {
        match self {
            Command::Create { subcommand } => subcommand.trailing_args(),
            Command::Read { args } => args,
            Command::Update { args } => args,
            Command::Delete { args } => args,
            Command::Validate { args } => args,
            Command::Generate { args } => args,
            Command::Build { args } => args,
            Command::Deploy { args } => args,
            Command::Config { args } => args,
            Command::Discover => &[],
            Command::Completions { .. } => &[],
            Command::Complete { .. } => &[],
        }
    }
}
