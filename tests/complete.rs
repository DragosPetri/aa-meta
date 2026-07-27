use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

/// Path to the attach-meta binary built by cargo.
fn attach_meta() -> std::path::PathBuf {
    env!("CARGO_BIN_EXE_attach-meta").into()
}

/// Write a temporary fake tool script that implements the minimal protocol
/// expected from attach-pickle: `discovery --json` and `complete <sub> <partial>`.
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
  "protocol_version": "0.1.0",
  "tool_name": "fake-attach-pickle",
  "tool_version": "0.1.0",
  "commands": {
    "create_node":     { "argv": ["create", "node"],     "supported": true },
    "create_property": { "argv": ["create", "property"], "supported": true },
    "read":            { "argv": ["read"],                "supported": true },
    "update":          { "argv": ["update"],              "supported": true },
    "delete_node":     { "argv": ["delete", "node"],     "supported": true },
    "delete_property": { "argv": ["delete", "property"], "supported": true },
    "validate":        { "argv": ["validate"],            "supported": true },
    "generate":        { "argv": ["generate"],            "supported": true },
    "build":           { "argv": ["build"],               "supported": false },
    "deploy":          { "argv": ["deploy"],              "supported": false },
    "config":          { "argv": ["config"],              "supported": true },
    "complete":        { "argv": ["complete"],            "supported": true }
  }
}
JSON
        ;;
    complete)
        # $2 = subcommand, $3 = partial word (may be empty)
        subcommand="$2"
        partial="$3"
        case "$subcommand" in
            create_node|create_property|delete_node|delete_property|read|update|validate)
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

/// Write a minimal .attach-meta.toml pointing at the fake tool.
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

fn run_complete(subcommand: &str, partial: &str) -> std::process::Output {
    let dir = tempfile::tempdir().unwrap();
    let tool = write_fake_tool(dir.path());
    let config = write_config(dir.path(), &tool);

    Command::new(attach_meta())
        .args(["--config", config.to_str().unwrap()])
        .args(["_complete", subcommand, partial])
        .output()
        .unwrap()
}

#[test]
fn complete_with_no_partial_returns_all_candidates() {
    let out = run_complete("create_node", "");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("node:temperature"));
    assert!(stdout.contains("node:pressure"));
    assert!(stdout.contains("node:humidity"));
    assert!(stdout.contains("primitive:threshold"));
}

#[test]
fn complete_filters_by_partial_word() {
    let out = run_complete("create_node", "node:");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("node:temperature"));
    assert!(stdout.contains("node:pressure"));
    assert!(stdout.contains("node:humidity"));
    assert!(!stdout.contains("primitive:threshold"));
}

#[test]
fn complete_returns_nothing_for_unmatched_partial() {
    let out = run_complete("create_node", "zzz");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.trim().is_empty());
}

#[test]
fn complete_respects_subcommand() {
    let out = run_complete("generate", "");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("artifact:binary"));
    assert!(stdout.contains("artifact:report"));
    assert!(!stdout.contains("node:temperature"));
}

#[test]
fn complete_is_silent_for_subcommand_with_no_candidates() {
    let out = run_complete("config", "");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // fake tool doesn't implement completions for 'config' — must exit 0 and print nothing
    assert!(out.status.success());
    assert!(stdout.trim().is_empty());
}
