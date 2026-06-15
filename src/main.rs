mod cli;
mod config;
mod discovery;
mod router;

use std::path::PathBuf;

use clap::{CommandFactory, Parser};
use clap_complete::{Shell, generate};
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();

    if let Some(shell) = cli.setup_completions {
        if let Err(e) = setup_completions(shell) {
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

    let config = config::load_config(cli.config.clone()).unwrap_or_else(|e| {
        eprintln!("attach-meta: config error: {e}");
        std::process::exit(1);
    });
    if let Err(e) = router::dispatch(command, cli.tool, cli.workfile, cli.json, config) {
        eprintln!("attach-meta: {e}");
        std::process::exit(1);
    }
}

fn setup_completions(shell: Shell) -> anyhow::Result<()> {
    let path = completion_path(shell)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut buf = Vec::new();
    generate(shell, &mut Cli::command(), "attach-meta", &mut buf);
    std::fs::write(&path, &buf)?;

    println!("Installed {} completions to {}", shell, path.display());
    maybe_print_hint(shell, &path);

    Ok(())
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
