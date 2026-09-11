use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "attach-meta",
    version,
    about = "Meta-tool for analog attachable tools",
    override_usage = "attach-meta [OPTIONS] [COMMAND] [ARGS]...",
    // Phase 1 only: we capture global flags + the subcommand name.
    // Everything after the subcommand is captured as raw trailing args
    // for phase-2 parsing (dynargs) once the manifest is loaded.
    trailing_var_arg = true,
    allow_hyphen_values = true,
    after_help = "\
Commands:
  init <binary>       Register an analog attachable tool
  completion <shell>  Print shell completion script (bash|zsh|fish)

Once a tool is registered, protocol commands (add, read, update, …) become available.
Run 'attach-meta init --help' for registration details.",
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

    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        hide = true
    )]
    pub args: Vec<String>,
}

pub fn parse_phase1() -> Cli {
    Cli::parse()
}
