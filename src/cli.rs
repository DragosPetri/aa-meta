use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "attach-meta",
    version,
    about = "Meta-tool for analog attachable tools",
    // Phase 1 only: we capture global flags + the subcommand name.
    // Everything after the subcommand is captured as raw trailing args
    // for phase-2 parsing (dynargs) once the manifest is loaded.
    trailing_var_arg = true,
    allow_hyphen_values = true,
)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        help = "Output raw/normalized JSON instead of human rendering"
    )]
    pub json: bool,

    #[arg(long, global = true, help = "Print verbose trace to stderr")]
    pub verbose: bool,

    /// The subcommand or positional arguments.
    /// Phase 1 captures everything here; main.rs splits off the command name.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

pub fn parse_phase1() -> Cli {
    Cli::parse()
}
