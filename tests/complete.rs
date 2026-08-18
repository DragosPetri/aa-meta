use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

fn attach_meta() -> std::path::PathBuf {
    env!("CARGO_BIN_EXE_attach-meta").into()
}

/// Write a fake tool that implements `--json discovery` and `__complete`.
fn write_fake_tool(dir: &std::path::Path) -> std::path::PathBuf {
    let script = dir.join("fake-attach-pickle");
    fs::write(
        &script,
        r#"#!/bin/sh
case "$1" in
    --json) shift ;;
esac
case "$1" in
    discovery)
        cat <<'JSON'
{
  "protocol_version": "0.5.0",
  "tool_name": "fake-attach-pickle",
  "tool_version": "0.1.0",
  "commands": {
    "create_node":     { "argv": ["add"],      "supported": true },
    "create_property": { "argv": ["set"],      "supported": true },
    "read_node":       { "argv": ["inspect"],  "supported": true },
    "read_property":   { "argv": ["get"],      "supported": true },
    "update":          { "argv": ["set"],      "supported": true },
    "delete_node":     { "argv": ["delete"],   "supported": true },
    "delete_property": { "argv": ["unset"],    "supported": true },
    "validate":        { "argv": ["validate"], "supported": true },
    "generate":        { "argv": ["generate"], "supported": true },
    "build":           { "argv": ["build"],    "supported": false },
    "deploy":          { "argv": ["deploy"],   "supported": false },
    "config":          { "argv": ["config"],   "supported": true },
    "create_workfile": { "argv": ["workfile"], "supported": true },
    "init":            { "argv": ["init"],     "supported": true },
    "list_devices":    { "argv": ["devices"],  "supported": true },
    "__complete":      { "argv": ["__complete"], "supported": true }
  }
}
JSON
        ;;
    __complete)
        shift  # remove __complete
        # parse --current-word-index, --, words...
        index=0
        while [ $# -gt 0 ]; do
            case "$1" in
                --current-word-index) index="$2"; shift 2 ;;
                --) shift; break ;;
                *) shift ;;
            esac
        done
        # $@ is now the word list; $index is the 0-based index of the word to complete
        i=0
        partial=""
        for w in "$@"; do
            if [ "$i" = "$index" ]; then
                partial="$w"
                break
            fi
            i=$((i + 1))
        done
        # context_count = tokens before the completing word
        context_count=$index
        # words[0] is the subcommand verb
        subcommand="$1"
        # words[0] is the tool's own native command (add/set/inspect/get/delete/unset/…)
        subcommand="$1"
        case "$subcommand" in
            add|set|inspect|get|delete|unset|validate|init|devices|workfile)
                echo "token_count:$context_count"
                for candidate in node:temperature node:pressure node:humidity primitive:threshold; do
                    case "$candidate" in
                        "$partial"*) echo "$candidate" ;;
                    esac
                done
                ;;
            generate)
                for candidate in artifact:binary artifact:report; do
                    case "$candidate" in
                        "$partial"*) echo "$candidate" ;;
                    esac
                done
                ;;
        esac
        ;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn write_config(dir: &std::path::Path, binary: &std::path::Path) -> std::path::PathBuf {
    let config = dir.join("config.toml");
    fs::write(
        &config,
        format!(
            "[meta]\ndefault_tool = \"fake-attach-pickle\"\n\n[[tools]]\nname = \"fake-attach-pickle\"\nbinary = \"{}\"\n",
            binary.display()
        ),
    )
    .unwrap();
    config
}

/// Run `attach-meta __complete` with the full simulated word list.
/// `words` is the complete shell word list as the shell would provide it, e.g.
/// ["attach-meta", "create", "node", "partial"].
/// `current_word_index` is the 0-based index of the word being completed.
fn run_double_complete(words: &[&str], current_word_index: usize) -> std::process::Output {
    let dir = tempfile::tempdir().unwrap();
    let tool = write_fake_tool(dir.path());
    let config = write_config(dir.path(), &tool);

    let index_str = current_word_index.to_string();
    let mut cmd = Command::new(attach_meta());
    cmd.args(["--config", config.to_str().unwrap()])
        .args(["__complete", "--current-word-index", &index_str])
        .arg("--")
        .args(words);
    cmd.output().unwrap()
}

#[test]
fn complete_at_index_2_returns_sub_verbs_from_discovery() {
    // attach-meta create <TAB>  — completing the second command word (index 2).
    // Flaw A fix: sub-verbs are derived from discovery argv arrays, not from __complete.
    let out = run_double_complete(&["attach-meta", "create", ""], 2);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "stdout: {stdout}");
    assert!(stdout.contains("node"), "expected 'node' in: {stdout}");
    assert!(stdout.contains("property"), "expected 'property' in: {stdout}");
    // Should NOT invoke __complete for this — no fake-tool candidates
    assert!(!stdout.contains("node:temperature"), "unexpected tool candidate in: {stdout}");
}

#[test]
fn complete_with_no_partial_at_positional_returns_all_candidates() {
    // attach-meta create node <TAB>  — completing the first positional (index 3).
    // Now index > 2, so delegates to __complete as before.
    let out = run_double_complete(&["attach-meta", "create", "node", ""], 3);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("node:temperature"));
    assert!(stdout.contains("node:pressure"));
    assert!(stdout.contains("node:humidity"));
    assert!(stdout.contains("primitive:threshold"));
}

#[test]
fn complete_filters_by_partial_word() {
    // attach-meta create node node:<TAB> — index 3; verb "create node" resolves to create_node
    let out = run_double_complete(&["attach-meta", "create", "node", "node:"], 3);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("node:temperature"));
    assert!(stdout.contains("node:pressure"));
    assert!(stdout.contains("node:humidity"));
    assert!(!stdout.contains("primitive:threshold"));
}

#[test]
fn complete_returns_no_candidates_for_unmatched_partial() {
    let out = run_double_complete(&["attach-meta", "create", "zzz"], 2);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(!stdout.contains("node:"));
    assert!(!stdout.contains("primitive:"));
}

#[test]
fn complete_respects_subcommand() {
    let out = run_double_complete(&["attach-meta", "generate", ""], 2);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("artifact:binary"));
    assert!(stdout.contains("artifact:report"));
    assert!(!stdout.contains("node:temperature"));
}

#[test]
fn complete_is_silent_for_subcommand_with_no_candidates() {
    let out = run_double_complete(&["attach-meta", "config", ""], 2);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.trim().is_empty());
}

#[test]
fn complete_forwards_positional_tokens() {
    // attach-meta create node already-typed-arg node:<TAB>  — index 4
    // Protocol "create node" (verb_count=2) → create_node native argv=["add"] (native_len=1)
    // adjusted_index = (4-1) - 2 + 1 = 2; tool receives ["add", "already-typed-arg", "node:"] at 2
    let out = run_double_complete(&["attach-meta", "create", "node", "already-typed-arg", "node:"], 4);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("token_count:2"), "expected token_count:2, got: {stdout}");
    assert!(stdout.contains("node:temperature"));
}

#[test]
fn complete_forwards_multiple_positional_tokens() {
    // attach-meta update tok0 tok1 tok2 <TAB>  — index 5
    let out = run_double_complete(&["attach-meta", "update", "tok0", "tok1", "tok2", ""], 5);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    // stripped index = 5 - 1 = 4 (update + tok0 + tok1 + tok2 precede the partial)
    assert!(stdout.contains("token_count:4"), "expected token_count:4, got: {stdout}");
}

#[test]
fn complete_at_index_1_returns_static_subcommand_list() {
    // attach-meta <TAB>  — completing the subcommand itself
    let out = run_double_complete(&["attach-meta", ""], 1);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("create"));
    assert!(stdout.contains("read"));
    assert!(stdout.contains("update"));
    assert!(stdout.contains("generate"));
    // should NOT invoke the tool at all (no tool lookup needed)
    assert!(!stdout.contains("node:temperature"));
}
