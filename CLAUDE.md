# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
cargo build                        # build
cargo test                         # run all tests
cargo test complete                # run a specific test by name filter
cargo install-local                # install binary to ~/.cargo/bin (alias in .cargo/config.toml)
attach-meta --setup-completions zsh  # install shell completions
```

## Architecture

`attach-meta` is a meta-tool that delegates commands to any registered "analog attachable" external tool. The active tool is resolved at runtime from config — nothing is hardcoded.

**Dispatch flow:**
1. `cli.rs` — parses the global flags (`--tool`, `--workfile`, `--json`, `--config`) and the subcommand
2. `config.rs` — loads config from (in priority order): `--config` flag → `.attach-meta.toml` walked up from cwd → `~/.config/attach-meta/config.toml`
3. `router.rs` — resolves the active tool from config, runs `<binary> --json discovery` to get its capability map, then calls the matching `argv` from the discovery response, forwarding `--json`, `--workfile`, and trailing args
4. `discovery.rs` — parses and validates the discovery JSON; enforces major-version match between the tool's `protocol_version` and attach-meta's own crate version

**The discovery protocol** — a compliant tool must respond to `<binary> --json discovery` with JSON matching `DiscoveryResponse` in `discovery.rs`: a `protocol_version`, `commands` map (each entry has `argv` and `supported`), and optional `settings`. The `argv` field is what gets prepended when the command is dispatched.

**Completions** — two modes:
- `attach-meta completions <shell>` prints a static clap-generated script (bash/fish/elvish/PowerShell)
- `attach-meta --setup-completions zsh` writes a dynamic zsh script that calls `attach-meta _complete <subcommand> <partial>` at completion time; `_complete` in turn calls `<binary> complete <subcommand> <partial>` after verifying the tool advertises `"complete"` in its discovery output

**Config format** (`.attach-meta.toml`):
```toml
[meta]
default_tool = "attach-pickle"

[[tools]]
name   = "attach-pickle"
binary = "attach-pickle"       # must be on PATH; absolute path also works
settings = {}                  # optional key/value forwarded to the tool
```

## Tests

Integration tests in `tests/complete.rs` spin up a fake shell-script tool that implements the minimal protocol, write a temp config pointing at it, and run the real binary via `CARGO_BIN_EXE_attach-meta`. There are no unit tests yet — all coverage is end-to-end via the binary.
