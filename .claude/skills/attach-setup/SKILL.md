---
name: attach-setup
description: Register an analog attachable binary with attach-meta (init + config session) and guide a user through configuring a device
---

# Attach Setup

You are guiding a user through one or both of these workflows using the `attach-meta` CLI:

1. **Init** — register an analog attachable binary, then drive an interactive config session with rich prompts and suggestions
2. **Configure** — set up a workfile and interact with it via CRUD commands

## Authoritative sources

Before interpreting command output or showing type shapes, run:
```
attach-meta schema responses
```
This prints the response schema JSON to stdout. Read it to understand every response type's fields, discriminators, and rendering hints. Never rely on memory for field names or type shapes.

## Detecting what the user wants

Ask the user:
- "Register a new tool?" → Phase 1 (Init)
- "Work with a registered tool / configure a device?" → Phase 2 (Configure)

If `.attach-meta.toml` exists in the current directory, check whether it already has a registered tool and skip directly to Phase 2 for that tool. A user can also do both phases in sequence.

---

## Phase 1 — Init

### Step 1: Identify the binary

Run `attach-meta --json suggest attachable` to discover known attachable binaries on PATH. Present the results to the user and let them pick one, or enter a custom binary name/path.

If the chosen binary is not on PATH, remind the user to add it or use the full path.

### Step 2: Register the tool (--no-interactive)

Run:
```
attach-meta --json init <binary> --no-interactive
```

Using `--no-interactive` lets you drive the config session yourself with richer guidance. `--json` is a global flag and must come before the subcommand.

Read the response schema (`attach-meta schema responses`) to parse the `InitResponse`:
- `ok: true` — registration succeeded; continue
- `ok: false` — show `message` and stop; the binary likely doesn't conform to the protocol — suggest the user run `/make-analog-attachable` to fix it
- `config_complete` — whether all required config fields are now set
- `missing_fields` — list of required field names that still need values

If init itself fails with a non-zero exit (transport error), show the error and stop.

### Step 3: Show the current config state

Run:
```
attach-meta --json tool-config-get
```

Read the response schema to parse `ToolConfigResponse`. Build a clear summary table:

| Field | Required | Type | Default | Current value | Status |
|---|---|---|---|---|---|
| `workfile_path` | yes | path | `/tmp/...` | *(not set)* | **needs value** |
| `log_level` | no | enum | `info` | `debug` | configured |

Interpretation rules are in the `Config` type description in the schema.

### Step 4: Drive the config session for required fields

For each required config where `value` is null:

1. Show the field name, description, and type
2. If `type` is `path`: suggest a sensible path based on the binary name and platform (e.g. `~/<binary-name>.workfile`); explain what the path should point to
3. If `type` is an enum (`type.options` array): list the valid options and suggest the most common one
4. If `type` is `numeric` or `string`: give a concrete example based on the description
5. If `default` is non-null: ask "Use default `<value>`? or enter your own:"
6. Once the user confirms a value, run:
   ```
   attach-meta --json tool-config-set <field_name> <value>
   ```
   If `ok: false`, show the error and re-prompt. If `ok: true`, mark the field as configured.

After all required fields are handled, re-run `attach-meta --json tool-config-get` to confirm every required field now has a non-null `value`.

### Step 5: Offer optional config fields

Show a summary of optional fields that are not yet set (value: null). Ask the user if they want to configure any — list them with descriptions.

For any the user chooses, apply the same prompting flow as Step 4.

### Step 6: Confirm init is complete

If all required fields are now set:
- Tell the user the tool is registered and ready
- Show the tool name and which binary it maps to
- Suggest the next step: "Run Phase 2 to create a workfile and start adding devices"

---

## Phase 2 — Configure a device

### Step 0: Verify registration

Run `attach-meta --json tool-config-get`. If this fails or returns `ok: false`, direct the user to Phase 1 first.

Show a brief summary: tool name, configured fields and their values.

**Read the manifest for command descriptions**: Locate the registered tool's manifest path in `.attach-meta.toml`, then read it (it is a JSON file). Each `CommandMapping` may carry a `description` string written by the tool author that explains what the command does for *this specific tool*. Use these descriptions throughout Phase 2 when explaining to the user what an action will do — prefer the tool-author's wording over generic protocol descriptions. If no description is present for a command, fall back to the protocol's own definition.

### Step 1: List available devices

Run:
```
attach-meta --json list-devices
```

Parse `ListDevicesResponse`. Show devices as: `key (tag)`. If empty, say so — the user will need a workfile first.

### Step 2: Create a workfile (if needed)

If list-devices returned empty or the user says there is no workfile yet:

Run:
```
attach-meta --json create-workfile
```

Parse `CreateWorkfileResponse`. Show the path of the created workfile. Note: the workfile path is stored in the tool's config — the tool manages it automatically.

Run `attach-meta --json list-devices` again to confirm the initial device list.

### Step 3: Interactive device workflow

Offer these actions in a loop. Ask the user what they want to do and execute it:

---

**Add a device**

Ask for:
- A key (unique device identifier — the tool's convention)
- Optionally a human-readable `--name`
- Optionally a parent path `--to` (space-separated `ValidIdentifier` segments from root; absent = root level)

Run:
```
attach-meta --json add <key> [--name <name>] [--to <parent...>]
```

Parse `AddResponse`. Show: "Added `<key>` at path `a → b → c`". If `ok: false`, show the error.

---

**Read the tree**

Ask which path to read (blank = full tree). Path is space-separated identifiers.

Run:
```
attach-meta --json read [<identifier>...]
```

Read the response schema for `ReadResponse` rendering hints — discriminate and render according to the type descriptions in the schema.

---

**Update/Insert a property**

Ask for:
- The full path to the property (space-separated from root; the last segment is the property name)
- The new value (passed as-is to the tool; the tool handles type coercion)

Run:
```
attach-meta --json update <path...> --with <value>
```

Parse `CommonResponse`. Confirm success or show the error. Note: `update` behaves like upsert — if the property doesn't exist, it is inserted under the resolved parent.

---

**Delete a node or property**

Ask for the path. Before running, warn the user: "This will delete the node and all its children."

First run without `--force` to see the preview:
```
attach-meta --json delete <path...>
```

Read the response schema for `DeleteResponse` discriminator — the type description explains how to distinguish a preview from a final result. Show the preview details and ask the user to confirm.

If confirmed, run with `--force`:
```
attach-meta --json delete <path...> --force
```

Confirm success or show the error.

---

**Validate the workfile**

Ask if the user wants to validate a specific path or the whole file (blank = all).

Run:
```
attach-meta --json validate [<path...>]
```

Read the response schema for `ValidationResponse` — its type description explains that it has no `ok` field and how to determine validity and render results.

---

**View current config**

Run `attach-meta --json tool-config-get` and show the current values.

---

**Change a config value**

Run the same prompting flow as Phase 1 Step 4 for the chosen field.

---

### Step 4: Suggest next steps

After each successful action, briefly suggest what makes sense next:
- After `add` → "Read the tree to confirm, validate the tree to confirm, or add properties with `update`"
- After `update` → "Read to verify the value, or `validate` to check for constraint errors"
- After `delete` → "Read or `validate` to see the current state"
- After a set of CRUD operations → "When done, run `validate` to check the full workfile"

---

## Important rules

- Always pass `--json` as a global flag before the subcommand: `attach-meta --json <subcommand> [args]` — this produces parseable JSON on stdout; without it, output goes to stderr in human-readable form and is harder to parse
- Always read the response schema (`attach-meta schema responses`) before interpreting a response — type descriptions contain discriminators, rendering hints, and semantic notes
- If `ok: false` on any response, always show the `message` field — it explains what went wrong
- Never show raw JSON to the user unless they ask for it — always render it as a readable summary
