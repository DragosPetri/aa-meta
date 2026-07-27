use std::path::PathBuf;

use clap::{Parser, Subcommand};
pub use clap_complete::Shell;

use crate::schema::DiscoveryKey;

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
    pub fn discovery_key(&self) -> DiscoveryKey {
        match self {
            CreateSubcommand::Node { .. } => DiscoveryKey::CreateNode,
            CreateSubcommand::Property { .. } => DiscoveryKey::CreateProperty,
        }
    }

    pub fn trailing_args(&self) -> &[String] {
        match self {
            CreateSubcommand::Node { args } | CreateSubcommand::Property { args } => args,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum DeleteSubcommand {
    #[command(about = "Delete a node")]
    Node {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    #[command(about = "Delete a property")]
    Property {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
}

impl DeleteSubcommand {
    pub fn discovery_key(&self) -> DiscoveryKey {
        match self {
            DeleteSubcommand::Node { .. } => DiscoveryKey::DeleteNode,
            DeleteSubcommand::Property { .. } => DiscoveryKey::DeleteProperty,
        }
    }

    pub fn trailing_args(&self) -> &[String] {
        match self {
            DeleteSubcommand::Node { args } | DeleteSubcommand::Property { args } => args,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum ReadSubcommand {
    #[command(about = "Read values of a node")]
    Node {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    #[command(about = "Read values of a property")]
    Property {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
}

impl ReadSubcommand {
    pub fn discovery_key(&self) -> DiscoveryKey {
        match self {
            ReadSubcommand::Node { .. } => DiscoveryKey::ReadNode,
            ReadSubcommand::Property { .. } => DiscoveryKey::ReadProperty,
        }
    }

    pub fn trailing_args(&self) -> &[String] {
        match self {
            ReadSubcommand::Node { args } | ReadSubcommand::Property { args } => args,
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
    #[command(about = "Read values of a node or property")]
    Read {
        #[command(subcommand)]
        subcommand: ReadSubcommand,
    },
    #[command(about = "Update primitive values")]
    Update {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    #[command(about = "Delete a node or property")]
    Delete {
        #[command(subcommand)]
        subcommand: DeleteSubcommand,
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
    #[command(about = "Print the protocol command table (--json for skeleton discovery JSON)")]
    Schema,
    #[command(about = "Print shell completion script for the given shell")]
    Completions { shell: Shell },
    #[command(name = "_complete", hide = true)]
    Complete {
        subcommand: String,
        partial: Option<String>,
    },
}

impl Command {
    pub fn key(&self) -> DiscoveryKey {
        match self {
            Command::Create { subcommand } => subcommand.discovery_key(),
            Command::Read { subcommand } => subcommand.discovery_key(),
            Command::Update { .. } => DiscoveryKey::Update,
            Command::Delete { subcommand } => subcommand.discovery_key(),
            Command::Validate { .. } => DiscoveryKey::Validate,
            Command::Generate { .. } => DiscoveryKey::Generate,
            Command::Build { .. } => DiscoveryKey::Build,
            Command::Deploy { .. } => DiscoveryKey::Deploy,
            Command::Config { .. } => DiscoveryKey::Config,
            // These variants are handled before dispatch is reached.
            Command::Discover | Command::Schema | Command::Completions { .. } | Command::Complete { .. } => {
                unreachable!("non-dispatch command reached key()")
            }
        }
    }

    pub fn trailing_args(&self) -> &[String] {
        match self {
            Command::Create { subcommand } => subcommand.trailing_args(),
            Command::Read { subcommand } => subcommand.trailing_args(),
            Command::Update { args } => args,
            Command::Delete { subcommand } => subcommand.trailing_args(),
            Command::Validate { args } => args,
            Command::Generate { args } => args,
            Command::Build { args } => args,
            Command::Deploy { args } => args,
            Command::Config { args } => args,
            Command::Discover => &[],
            Command::Schema => &[],
            Command::Completions { .. } => &[],
            Command::Complete { .. } => &[],
        }
    }
}
