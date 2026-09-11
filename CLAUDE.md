# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
cargo build                        # build
cargo test                         # run all tests (unit + integration)
cargo test --bin attach-meta       # run unit tests only
cargo test --test integration      # run integration tests only
cargo install-local                # install binary to ~/.cargo/bin (alias in .cargo/config.toml)
attach-meta completion zsh         # print zsh completion script to stdout
```

## Architecture

`attach-meta` is a meta-tool that delegates commands to any registered "analog attachable" external tool. Tools are registered via manifests, validated against a shipped JSON Schema, and dispatched via subprocess.

**Registration flow:**
1. `attach-meta init <binary>` calls `<binary> attach-manifest`
2. The tool writes a manifest file to disk and prints its path to stdout
3. attach-meta validates the manifest against `schemas/manifest.schema.json` (JSON Schema draft 2020-12)
4. Checks `protocol_version` major matches attach-meta's own major version (strict major-match, not 0.x-minor)
5. Writes binary path, manifest path, and SHA-256 hash to `.attach-meta.toml`

**Dispatch flow (two-phase arg parsing):**
1. `cli.rs` — Phase 1: parses global flags (`--json`, `--verbose`) and captures the command name + remaining args as raw `Vec<String>`
2. `config.rs` — loads config from `.attach-meta.toml` walked up from cwd → default cwd
3. `manifest_store.rs` — reads manifest from stored path, compares SHA-256 hash:
   - unchanged → parse only, skip validation
   - changed → full meta-schema + version validation; persist new hash on success
   - missing/unreadable → hard error
4. `dynargs.rs` — Phase 2: builds runtime flag set from protocol base schema + tool-declared `args`, parses raw args into flags JSON + positionals
5. `schema.rs` — validates input against `allOf(base_schema, tool.args)` effective schema
6. `commands/` — dispatches to the appropriate handler which calls `transport.rs` to invoke the subprocess

**Protocol commands (kebab-case):**
- Required: `tool-config-get`, `tool-config-set`, `create-workfile`, `list-devices`, `add`, `read`, `update`, `delete`, `validate`
- Optional (convenience): `move`, `rename`, `alias` — require native tool support; no built-in fallbacks
- Optional (pipeline): `generate`, `build`, `deploy`
- Optional (intelligence): `list-intelligence`, `suggest`
- Meta: `init`, `completion`, `__complete`

**Completions** — two modes:
- `attach-meta completion <bash|zsh|fish>` prints a script that calls `attach-meta __complete -- <subcommand> [args...] <partial>`
- `__complete` is a shell adapter over `suggest` (spec §Completions steps 1–7)

**Exit codes:**
- `0` = `ok:true` in response
- `1` = well-formed response with `ok:false` (protocol failure)
- `2` = input/manifest/transport error (attach-meta's error envelope)

**Config format** (`.attach-meta.toml`):
```toml
[meta]
default_tool = "attach-pickle"

[[tools]]
name = "attach-pickle"
binary = "attach-pickle"
manifest_path = "/path/to/manifest.json"
manifest_sha256 = "abc123..."
settings = {}
```

## Source layout

```
src/
  main.rs              entry, two-phase arg handling, top-level dispatch, exit-code mapping
  error.rs             AttachMetaError + ErrorEnvelope + exit_code()
  cli.rs               clap derive Phase 1 (global flags + trailing_var_arg capture)
  dynargs.rs           Phase 2 runtime flag builder from manifest args; CLI→JSON + forwarded argv
  config.rs            .attach-meta.toml load/save, walk-up precedence, multi-tool registry
  schema.rs            jsonschema (draft 2020-12) wrappers: manifest meta-schema + input validation
  manifest_store.rs    hash re-read rule (sha256 compare → validate-on-change → persist)
  transport.rs         subprocess-per-command, timeout kill, capture, error classification
  prompt.rs            Prompter trait (StdinPrompter + ScriptedPrompter) for interactive init
  render.rs            human rendering per response type + --json passthrough
  complete.rs          __complete adapter + completion-script generation/installer
  protocol/
    mod.rs
    manifest.rs        Manifest, CommandMapping, CommandName enum, CompletionHint
    version.rs         SemVer parse + check_major_match (strict major)
    base_schema.rs     hardcoded base schema per command + effective_schema (allOf merge)
    responses.rs       all response types from responses.schema.json as serde types
  commands/
    mod.rs             CommandContext + CommandName→handler dispatch
    init.rs            init flow + interactive config session
    config.rs          tool-config-get / tool-config-set
    workspace.rs       create-workfile, list-devices
    crud.rs            add, read, update, delete
    restructure.rs     move, rename, alias (native dispatch only)
    pipeline.rs        generate, build, deploy
    validate.rs        validate + path-prefix filtering
    intelligence.rs    list-intelligence, suggest
docs/schemas/          manifest.schema.json + responses.schema.json (embedded via include_str!)
  command_base_schemas/  per-command base input schemas (add, read, update, etc.) merged with tool args via allOf
```

## Tests

- **Unit tests** (31): serde round-trips for protocol types, version parsing, manifest meta-schema validation (valid/invalid/missing commands/forbidden required), input validation, dynargs parsing
- **Integration tests** (`tests/integration.rs`, 33): spin up a fake shell-script tool that implements `attach-manifest` and returns canned responses per command. Tests cover: init flow, version mismatch, meta-schema rejection, hash caching, input validation, CRUD operations, validate filtering, transport errors (non-zero/empty/malformed/timeout), protocol errors (ok:false), exit codes, move/rename not-in-manifest errors, completions, --json mode

## Key dependencies

- `jsonschema` 0.28 (draft 2020-12) — manifest meta-schema and input validation
- `sha2` — manifest content hashing
- `clap` (derive + builder) — Phase 1 CLI parsing
- `serde`/`serde_json`/`toml` — serialization (toml now used for both read and write)
