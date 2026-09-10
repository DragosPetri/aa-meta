---
name: make-analog-attachable
description: Guide the user through making an external binary conform to the attach-meta protocol (Analog Attachable)
---

# Make Analog Attachable

You are helping the user make an external binary into an Analog Attachable — a tool that conforms to the attach-meta protocol and can be registered via `attach-meta init <binary>`.

## Authoritative sources

The JSON Schema files in this repository are the single source of truth for all types. Never duplicate or paraphrase type definitions — always read and reference the schema files directly:

- **Manifest structure**: `docs/schemas/manifest.schema.json` — defines the manifest the tool must write during `attach-manifest`
- **Response types**: `docs/schemas/responses.schema.json` — defines every response type (`$defs`) the tool must produce
- **Command base input schemas**: `docs/schemas/command_base_schemas/<command>.schema.json` — defines the protocol-owned input flags for each command; the tool's `args` are merged via `allOf`
- **Spec**: `docs/spec.txt` — prose protocol specification with behavioral rules and NOTEs

Before answering any question about a type shape, required fields, or allowed values: read the relevant schema file. Do not rely on memory or summaries.

## Workflow

### Step 1 — Understand and audit the tool

Ask the user:
1. What binary/command is the tool? (e.g. `attach-pickle`, `attach-dts`)
2. What data format does it work with? (e.g. pickle files, devicetree source)
3. What language is the tool written in?

Then **read the tool's source code** to understand what it already has. Look for:
- Existing CLI subcommands or argument parsing
- An existing manifest or `attach-manifest` command
- Config read/write mechanisms
- CRUD-like operations on a tree or structured data
- Validation logic
- Build/generate/deploy steps
- Anything that already produces JSON on stdout

### Step 2 — Gap analysis

Read `docs/schemas/manifest.schema.json` to determine which commands are required and which are optional.

Compare what the tool already has against the protocol requirements. Produce a concrete gap report:
- **Already conformant**: commands/behaviors that already match the protocol contract (correct response shape, exit code semantics, JSON on stdout)
- **Partially conformant**: existing functionality that needs adaptation (e.g. has CRUD but wrong response shape, has config but not as `Config` objects)
- **Missing**: commands with no existing equivalent that need to be written from scratch

For partially conformant commands, identify the specific delta — is it just the response shape? Missing fields? Wrong exit code semantics? This determines whether the work is a thin wrapper or a real implementation.

### Step 3 — Implement `attach-manifest` (if missing)

This is the only command required for registration. When the binary is called with `attach-manifest`:

1. Write a manifest JSON file to a stable path on disk
2. Print that path (and nothing else) to stdout
3. Exit 0

Read `docs/schemas/manifest.schema.json` and walk the user through it. The meta-schema is the authority on which commands are required vs optional, what fields a `CommandMapping` needs, and what constraints apply (e.g. forbidden top-level `required` in tool `args`). Do not enumerate these — point the user at the schema and explain what they're reading.

Help the user write a concrete manifest for their tool. For commands the tool already implements, wire the existing binary/subcommand into the manifest's `argv`. For commands that need new implementation, decide on the argv routing together with the user.

**Command descriptions**: Encourage the user to add a `description` string to every `CommandMapping`. This is optional but important for agents — it tells attach-setup and other agents what the command does in the *context of this specific tool* (e.g. `"add"` on a pickle tool might say "Insert a new named object into the pickle store"). Generic descriptions ("Adds a device") are fine; tool-specific ones are better. Example:

```json
"add": {
  "description": "Insert a new named object into the pickle store at the given path.",
  "argv": ["attach-pickle", "add"]
}
```

**Protocol-owned flags**: For each command being added to the manifest, read its base schema from `docs/schemas/command_base_schemas/<command>.schema.json` and tell the user what flags the protocol already provides. These are parsed by attach-meta and forwarded automatically — the tool must handle them but must NOT redeclare them in `args` (collisions are a hard error). The tool's `args` field is only for *additional* optional flags specific to the tool; omit it when the tool has none.

**Suggest completions early**: Even if the user isn't planning to implement `suggest` yet, encourage adding `completions` hints as comments in the manifest so they're easy to wire up later. For example:

```jsonc
"add": {
  "argv": ["my-tool", "add"],
  // "completions": [
  //   { "arg": "add",    "kind": "device-key" },  // positional arg
  //   { "arg": "to",     "kind": "node-key"   },  // array flag (--to)
  //   { "arg": "bus_id", "kind": "bus-id"     }   // string flag (--bus_id)
  // ]
}
```

This reminds the tool author which arguments are completable once `list-intelligence` and `suggest` are implemented. The `arg` field has three roles:

- **Positional args** (bare args like `<Device.key>` in `add` or `path...` in `read`): use the **command name** as `arg`.
- **Array flag values** (e.g. `--to`): use the **flag name** (without `--`) as `arg`. Completions are called with previously-provided values as context, growing as each value is added.
- **String/bool flag values** (e.g. `--bus_id`): use the **flag name** (without `--`) as `arg`.

The `kind` must match a kind advertised by `list-intelligence`.

### Step 4 — Close the gaps

Work through the gap report from Step 2. For each gap:

1. Read the base input schema from `docs/schemas/command_base_schemas/<command>.schema.json` — this shows what flags/positionals the protocol sends
2. Read the response type from `docs/schemas/responses.schema.json` (look up the `$defs` entry for the command's response type)
3. Review the behavioral rules in `docs/spec.txt` for that command

For **partially conformant** commands, adapt the existing implementation — wrap the output into the correct response shape, adjust exit code handling, add missing fields. Prefer thin adapter layers over rewrites.

For **missing** commands, implement them. Prioritize in this order:
1. Config (`tool-config-get`, `tool-config-set`) — needed for init to complete
2. Workspace (`create-workfile`, `list-devices`)
3. CRUD (`add`, `read`, `update`, `delete`) — the core data loop
4. Validation (`validate`)
5. Optional commands only if the user wants them — convenience (`move`, `rename`, `alias`), pipeline (`generate`, `build`, `deploy`), intelligence (`list-intelligence`, `suggest`)

**Transport contract** (applies to every command):
- Exit 0: stdout must contain valid JSON matching the command's response type
- Non-zero exit: operation failed; stderr should have a human-readable reason; stdout is ignored
- `ok: false` with exit 0 means the tool ran but the operation failed (protocol failure, not transport failure)

### Step 5 — Test with attach-meta

Guide the user to:
1. `attach-meta init <binary>` — registers the tool
2. Verify with `attach-meta tool-config-get`
3. Walk through the CRUD cycle: `create-workfile` → `add` → `read` → `update` → `read` → `delete`
4. Run `attach-meta validate`

### Step 6 — Optional: completions and intelligence

If the user wants tab-completion support:
1. Add `completions` entries to relevant commands in the manifest
2. Implement `list-intelligence` and `suggest`
3. Read the `__complete` dispatch rules in `docs/spec.txt` §Completions to understand how attach-meta bridges shell TAB to `suggest`

## Important rules

- Always read the schema files before showing type shapes — they are the source of truth
- The tool's `args` may only add optional properties (no top-level `required`)
- `ok: true` + `severity: "error"` is a protocol invariant violation — must never occur
- `ValidationResponse` does NOT extend `CommonResponse` — it has only `errors` and `warnings`
- `ReadResponse` does NOT extend `CommonResponse` — it is oneOf Node or Property
- Config `default` must always be present; `null` means no default (not a valid default value)
- `ValidIdentifier` paths are resolved left-to-right from root; aliases are valid only if the tool declares alias support
