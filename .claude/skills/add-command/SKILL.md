---
name: add-command
description: Add or update a command in the attach-meta protocol
---

Use this checklist whenever a command is added, renamed, or removed. Every step is required — the pieces are coupled and drift silently if any is skipped.

## Files to touch

### 1. `src/schema.rs` — the authoritative table
Two coupled changes are required:

**a) Add a variant to `DiscoveryKey`** (e.g. `UpdateNode`). This enum is the exhaustiveness anchor — every variant must have a `spec()` arm or the code won't compile.

**b) Add the matching arm to `DiscoveryKey::spec()`** returning a `CommandSpec`. Fields:
- `key` — the discovery key the tool must advertise (e.g. `"update_node"`)
- `argv` — the argv the tool receives (e.g. `&["update", "node"]`)
- `description` — one sentence, shown in `attach-meta schema`

**c) Add the variant to `DiscoveryKey::ALL`** so it appears in `attach-meta schema` output. This is the one manual step the compiler does not enforce — the `all_keys_are_unique` test guards against duplicates but not omissions.

### 2. `src/cli.rs` — CLI shape
- If the command has subcommands (e.g. `node`/`property`), add a `*Subcommand` enum with `discovery_key()` returning `DiscoveryKey` and `trailing_args()`, matching the pattern of `CreateSubcommand`.
- Add the variant to `Command` enum.
- Add arms to `Command::key()` (returning `DiscoveryKey`) and `Command::trailing_args()`.

### 3. `src/main.rs` — zsh completions
- Commands with flat trailing args: add the discovery key to the `dynamic` array in `generate_zsh_script`.
- Commands with typed subcommands: add a `*_case` block (following the `create_case` / `read_case` / `delete_case` pattern) and include it in the format string.
- Update the command description string in the zsh `commands=(...)` list.

### 4. `Cargo.toml` — protocol version
The protocol version equals the crate version. Bump it according to semver (under 1.0, minor = breaking):
- Adding a new optional command → **patch** bump
- Adding a required command or changing an existing key/argv → **minor** bump (breaking)
- Removing a command → **minor** bump (breaking)

### 5. `tests/complete.rs` — fake tool
- Update the discovery JSON in `write_fake_tool` to include the new/updated command keys.
- Update the `complete` handler's `case` pattern to cover the new key if it should return candidates.
- If the protocol version changed, update `"protocol_version"` in the fake tool's discovery JSON.

## Verification

Run `cargo test` — all integration tests must pass.

Spot-check the output:
```
attach-meta schema           # human-readable table — new entry should appear
attach-meta --json schema    # skeleton JSON — new key should appear
```
