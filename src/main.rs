mod cli;
mod config;
mod discovery;
mod router;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();
    let config = config::load_config(cli.config.clone()).unwrap_or_else(|e| {
        eprintln!("attach-meta: config error: {e}");
        std::process::exit(1);
    });
    if let Err(e) = router::dispatch(cli, config) {
        eprintln!("attach-meta: {e}");
        std::process::exit(1);
    }
}
