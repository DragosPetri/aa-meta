use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn attach_meta() -> PathBuf {
    env!("CARGO_BIN_EXE_attach-meta").into()
}

struct TestEnv {
    dir: tempfile::TempDir,
    tool_path: PathBuf,
    config_path: PathBuf,
    manifest_path: PathBuf,
    log_path: PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let tool_path = dir.path().join("fake-tool");
        let manifest_path = dir.path().join("manifest.json");
        let config_path = dir.path().join(".attach-meta.toml");
        let log_path = dir.path().join("invocations.log");

        Self {
            dir,
            tool_path,
            config_path,
            manifest_path,
            log_path,
        }
    }

    fn write_manifest(&self, manifest_json: &str) {
        fs::write(&self.manifest_path, manifest_json).unwrap();
    }

    fn write_default_manifest(&self) {
        self.write_manifest(&format!(
            r#"{{
  "protocol_version": "1.0.0",
  "commands": {{
    "tool-config-get": {{ "argv": ["{bin}", "config-get"] }},
    "tool-config-set": {{ "argv": ["{bin}", "config-set"] }},
    "create-workfile": {{ "argv": ["{bin}", "workfile"] }},
    "list-devices":    {{ "argv": ["{bin}", "devices"] }},
    "add":             {{ "argv": ["{bin}", "add"], "args": {{ "properties": {{ "bus_id": {{ "type": "string" }} }} }}, "completions": [{{ "arg": "add", "kind": "device-key" }}] }},
    "read":            {{ "argv": ["{bin}", "read"] }},
    "update":          {{ "argv": ["{bin}", "update"] }},
    "delete":          {{ "argv": ["{bin}", "delete"] }},
    "validate":        {{ "argv": ["{bin}", "validate"] }},
    "generate":        {{ "argv": ["{bin}", "generate"] }},
    "build":           {{ "argv": ["{bin}", "build"] }},
    "deploy":          {{ "argv": ["{bin}", "deploy"] }},
    "list-intelligence": {{ "argv": ["{bin}", "list-intelligence"] }},
    "suggest":         {{ "argv": ["{bin}", "suggest"] }}
  }}
}}"#,
            bin = self.tool_path.display()
        ));
    }

    fn write_tool(&self, extra_cases: &str) {
        let script = format!(
            r#"#!/bin/sh
LOG="{log}"
echo "$0 $*" >> "$LOG"

case "$1" in
    attach-manifest)
        echo "{manifest}"
        exit 0
        ;;{extra_cases}
    config-get)
        cat <<'JSON'
{{"ok":true,"message":"configs","severity":"info","configs":[
  {{"field_name":"workfile","description":"Path to workfile","type":"path","required":true,"default":null,"value":null}},
  {{"field_name":"board","description":"Target board","type":"string","required":false,"default":"generic","value":"generic"}}
]}}
JSON
        exit 0
        ;;
    config-set)
        echo '{{"ok":true,"message":"config set","severity":"info"}}'
        exit 0
        ;;
    workfile)
        echo '{{"ok":true,"message":"workfile created","severity":"info","path":"/tmp/test.dts"}}'
        exit 0
        ;;
    devices)
        echo '{{"ok":true,"message":"devices","severity":"info","devices":[{{"tag":"sensor","key":"ad7124"}},{{"tag":"sensor","key":"ad5940"}}]}}'
        exit 0
        ;;
    add)
        shift
        echo '{{"ok":true,"message":"added","severity":"info","key":"new_node","path":["root","new_node"]}}'
        exit 0
        ;;
    read)
        shift
        if [ "$1" = "my_node" ] && [ "$2" = "temperature" ]; then
            echo '{{"kind":"property","key":"temperature","type":{{"kind":"number","subtype":"int"}},"value":25}}'
        else
            echo '{{"kind":"node","key":"root","properties":[{{"kind":"property","key":"temp","type":{{"kind":"number","subtype":"int"}},"value":42}}],"children":[{{"kind":"node","key":"child1","properties":[],"children":[]}}]}}'
        fi
        exit 0
        ;;
    update)
        echo '{{"ok":true,"message":"updated","severity":"info"}}'
        exit 0
        ;;
    delete)
        shift
        has_force=false
        for arg in "$@"; do
            if [ "$arg" = "--force" ]; then has_force=true; fi
        done
        if [ "$has_force" = "true" ]; then
            echo '{{"ok":true,"message":"deleted","severity":"info"}}'
        else
            echo '{{"ok":true,"message":"preview","severity":"info","node_count":3,"property_count":5,"paths":[["a","b"],["a","c"]]}}'
        fi
        exit 0
        ;;
    validate)
        echo '{{"errors":[{{"kind":"generic","path":["soc","spi"],"message":"clock too high"}},{{"kind":"generic","path":[],"message":"file-level warning"}}],"warnings":[{{"kind":"generic","path":["soc"],"message":"deprecated"}}]}}'
        exit 0
        ;;
    generate)
        echo '{{"ok":true,"message":"generated","severity":"info"}}'
        exit 0
        ;;
    build)
        echo '{{"ok":true,"message":"built","severity":"info"}}'
        exit 0
        ;;
    deploy)
        echo '{{"ok":true,"message":"deployed","severity":"info"}}'
        exit 0
        ;;
    list-intelligence)
        echo '{{"ok":true,"message":"intelligence","severity":"info","intelligence":[{{"kind":"device-key","args":[{{"name":"parent","description":"parent node","required":false,"kind":"node-key"}}]}},{{"kind":"node-key","args":[]}}]}}'
        exit 0
        ;;
    suggest)
        shift
        echo '{{"ok":true,"message":"suggestions","severity":"info","suggestions":[{{"value":"ad7124"}},{{"value":"ad5940"}}]}}'
        exit 0
        ;;
    *)
        echo "unknown command: $1" >&2
        exit 1
        ;;
esac
"#,
            log = self.log_path.display(),
            manifest = self.manifest_path.display(),
            extra_cases = extra_cases,
        );
        fs::write(&self.tool_path, script).unwrap();
        fs::set_permissions(&self.tool_path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn write_default_tool(&self) {
        self.write_tool("");
    }

    fn write_config_pointing_to_tool(&self) {
        let hash = sha256_file(&self.manifest_path);
        let config = format!(
            r#"[meta]
default_tool = "fake-tool"

[[tools]]
name = "fake-tool"
binary = "{bin}"
manifest_path = "{manifest}"
manifest_sha256 = "{hash}"
"#,
            bin = self.tool_path.display(),
            manifest = self.manifest_path.display(),
            hash = hash,
        );
        fs::write(&self.config_path, config).unwrap();
    }

    fn run_init(&self) -> std::process::Output {
        Command::new(attach_meta())
            .args([
                "--config",
                self.config_path.to_str().unwrap(),
                "init",
                self.tool_path.to_str().unwrap(),
                "--no-interactive",
            ])
            .output()
            .unwrap()
    }

    fn run_cmd(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = Command::new(attach_meta());
        cmd.args(["--config", self.config_path.to_str().unwrap()]);
        cmd.args(args);
        cmd.output().unwrap()
    }

    fn run_json(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = Command::new(attach_meta());
        cmd.args(["--config", self.config_path.to_str().unwrap(), "--json"]);
        cmd.args(args);
        cmd.output().unwrap()
    }

    fn invocation_log(&self) -> String {
        fs::read_to_string(&self.log_path).unwrap_or_default()
    }
}

fn sha256_file(path: &Path) -> String {
    use std::io::Read;
    let mut file = fs::File::open(path).unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    sha256_hex(&content)
}

fn sha256_hex(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

// ─── Init ───

#[test]
fn init_no_interactive_writes_toml() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();

    let out = env.run_init();
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let toml_content = fs::read_to_string(&env.config_path).unwrap();
    assert!(
        toml_content.contains("manifest_path"),
        "toml: {toml_content}"
    );
    assert!(
        toml_content.contains("manifest_sha256"),
        "toml: {toml_content}"
    );
    assert!(toml_content.contains("fake-tool"), "toml: {toml_content}");
}

#[test]
fn init_returns_init_response_json() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();

    let out = Command::new(attach_meta())
        .args([
            "--config",
            env.config_path.to_str().unwrap(),
            "--json",
            "init",
            env.tool_path.to_str().unwrap(),
            "--no-interactive",
        ])
        .output()
        .unwrap();

    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["ok"], true);
    assert!(response["missing_fields"].is_array());
}

// ─── Version mismatch ───

#[test]
fn init_rejects_major_mismatch() {
    let env = TestEnv::new();
    env.write_manifest(
        r#"{
        "protocol_version": "99.0.0",
        "commands": {
            "tool-config-get": { "argv": ["t", "cg"] },
            "tool-config-set": { "argv": ["t", "cs"] },
            "create-workfile": { "argv": ["t", "w"] },
            "list-devices": { "argv": ["t", "d"] },
            "add": { "argv": ["t", "a"] },
            "read": { "argv": ["t", "r"] },
            "update": { "argv": ["t", "u"] },
            "delete": { "argv": ["t", "del"] },
            "validate": { "argv": ["t", "v"] }
        }
    }"#,
    );
    env.write_default_tool();

    let out = env.run_init();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("mismatch") || stderr(&out).contains("version"));
}

// ─── Meta-schema rejection ───

#[test]
fn init_rejects_missing_required_command() {
    let env = TestEnv::new();
    // Missing "add" command
    env.write_manifest(
        r#"{
        "protocol_version": "1.0.0",
        "commands": {
            "tool-config-get": { "argv": ["t", "cg"] },
            "tool-config-set": { "argv": ["t", "cs"] },
            "create-workfile": { "argv": ["t", "w"] },
            "list-devices": { "argv": ["t", "d"] },
            "read": { "argv": ["t", "r"] },
            "update": { "argv": ["t", "u"] },
            "delete": { "argv": ["t", "del"] },
            "validate": { "argv": ["t", "v"] }
        }
    }"#,
    );
    env.write_default_tool();

    let out = env.run_init();
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn init_rejects_required_in_tool_args() {
    let env = TestEnv::new();
    env.write_manifest(r#"{
        "protocol_version": "1.0.0",
        "commands": {
            "tool-config-get": { "argv": ["t", "cg"] },
            "tool-config-set": { "argv": ["t", "cs"] },
            "create-workfile": { "argv": ["t", "w"] },
            "list-devices": { "argv": ["t", "d"] },
            "add": { "argv": ["t", "a"], "args": { "properties": { "x": {"type":"string"} }, "required": ["x"] } },
            "read": { "argv": ["t", "r"] },
            "update": { "argv": ["t", "u"] },
            "delete": { "argv": ["t", "del"] },
            "validate": { "argv": ["t", "v"] }
        }
    }"#);
    env.write_default_tool();

    let out = env.run_init();
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("required"),
        "expected 'required' error, got: {err}"
    );
}

// ─── Hash caching ───

#[test]
fn hash_change_triggers_revalidation() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    // First command should work (hash matches)
    let out = env.run_json(&["list-devices"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    // Mutate manifest (add a space)
    let content = fs::read_to_string(&env.manifest_path).unwrap();
    fs::write(&env.manifest_path, format!("{content} ")).unwrap();

    // Next command should re-validate and update hash
    let out = env.run_json(&["list-devices"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let toml_content = fs::read_to_string(&env.config_path).unwrap();
    let new_hash = sha256_file(&env.manifest_path);
    assert!(
        toml_content.contains(&new_hash),
        "hash not updated in config"
    );
}

// ─── Input validation ───

#[test]
fn add_without_key_or_name_is_rejected() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["add"]);
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));

    // Should NOT have spawned the subprocess
    assert!(env.invocation_log().is_empty() || !env.invocation_log().contains(" add"));
}

#[test]
fn add_with_key_succeeds() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["add", "my_device"]);
    assert!(
        out.status.success(),
        "stdout: {} stderr: {}",
        stdout(&out),
        stderr(&out)
    );
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["ok"], true);
    assert_eq!(response["key"], "new_node");
}

#[test]
fn add_with_tool_declared_flag() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["add", "k", "--bus_id", "spi0"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let log = env.invocation_log();
    assert!(
        log.contains("--bus_id"),
        "tool should receive --bus_id, log: {log}"
    );
    assert!(log.contains("spi0"), "tool should receive spi0, log: {log}");
}

// ─── CRUD ───

#[test]
fn read_root_returns_node_tree() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["read"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["kind"], "node");
    assert_eq!(response["key"], "root");
}

#[test]
fn read_property_path() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["read", "my_node", "temperature"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["kind"], "property");
    assert_eq!(response["value"], 25);
}

#[test]
fn update_requires_with_flag() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["update", "some_prop"]);
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn update_with_value_succeeds() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["update", "my_prop", "--with", "42"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
}

#[test]
fn delete_without_force_returns_preview() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["delete", "some_node"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert!(
        response.get("node_count").is_some(),
        "expected DeletePreview"
    );
}

#[test]
fn delete_with_force_returns_common() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["delete", "some_node", "--force"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["ok"], true);
    assert!(response.get("node_count").is_none());
}

// ─── Validate filtering ───

#[test]
fn validate_no_path_returns_all() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["validate"]);
    // validate has errors[] so exit 0 (non-empty errors don't affect exit for validate)
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    let errors = response["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 2);
}

#[test]
fn validate_with_path_filters_by_prefix() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["validate", "soc", "spi"]);
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    let errors = response["errors"].as_array().unwrap();
    // Should include "soc/spi" error + file-level error (path: [])
    assert_eq!(errors.len(), 2, "errors: {errors:?}");

    let warnings = response["warnings"].as_array().unwrap();
    // "soc" alone is NOT prefixed by ["soc","spi"] so it's filtered out
    assert_eq!(warnings.len(), 0, "warnings: {warnings:?}");
}

#[test]
fn validate_file_level_always_passes_through() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["validate", "nonexistent"]);
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    let errors = response["errors"].as_array().unwrap();
    // Only the file-level error (path: []) should pass through
    assert_eq!(errors.len(), 1);
    assert!(errors[0]["path"].as_array().unwrap().is_empty());
}

// ─── Transport errors ───

#[test]
fn transport_error_nonzero_exit() {
    let env = TestEnv::new();
    env.write_default_manifest();
    // Tool that returns non-zero for "generate"
    env.write_tool(
        r#"
    generate)
        echo "something went wrong" >&2
        exit 1
        ;;
"#,
    );
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["generate"]);
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["ok"], false);
    assert_eq!(response["category"], "transport");
}

#[test]
fn transport_error_empty_stdout() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_tool(
        r#"
    build)
        exit 0
        ;;
"#,
    );
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["build"]);
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert!(response["error"].as_str().unwrap().contains("no output"));
}

#[test]
fn transport_error_malformed_json() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_tool(
        r#"
    deploy)
        echo "not json at all"
        exit 0
        ;;
"#,
    );
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["deploy"]);
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert!(response["error"].as_str().unwrap().contains("JSON"));
}

#[test]
fn transport_timeout() {
    let env = TestEnv::new();
    // Manifest with a 200ms timeout on generate
    env.write_manifest(&format!(
        r#"{{
  "protocol_version": "1.0.0",
  "commands": {{
    "tool-config-get": {{ "argv": ["{bin}", "config-get"] }},
    "tool-config-set": {{ "argv": ["{bin}", "config-set"] }},
    "create-workfile": {{ "argv": ["{bin}", "workfile"] }},
    "list-devices":    {{ "argv": ["{bin}", "devices"] }},
    "add":             {{ "argv": ["{bin}", "add"] }},
    "read":            {{ "argv": ["{bin}", "read"] }},
    "update":          {{ "argv": ["{bin}", "update"] }},
    "delete":          {{ "argv": ["{bin}", "delete"] }},
    "validate":        {{ "argv": ["{bin}", "validate"] }},
    "generate":        {{ "argv": ["{bin}", "generate"], "timeout_ms": 200 }}
  }}
}}"#,
        bin = env.tool_path.display()
    ));
    env.write_tool(
        r#"
    generate)
        sleep 5
        echo '{"ok":true,"message":"done","severity":"info"}'
        exit 0
        ;;
"#,
    );
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["generate"]);
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert!(response["error"].as_str().unwrap().contains("timed out"));
}

// ─── Protocol error (ok:false at exit 0) ───

#[test]
fn protocol_error_ok_false() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_tool(
        r#"
    generate)
        echo '{"ok":false,"message":"generation failed","severity":"error"}'
        exit 0
        ;;
"#,
    );
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["generate"]);
    assert_eq!(out.status.code(), Some(1), "ok:false should exit 1");
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["ok"], false);
}

// ─── Pipeline commands ───

#[test]
fn generate_build_deploy() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    for cmd in ["generate", "build", "deploy"] {
        let out = env.run_json(&[cmd]);
        assert!(out.status.success(), "{cmd} failed: {}", stderr(&out));
    }
}

// ─── Completions ───

#[test]
fn completion_bash_prints_script() {
    let out = Command::new(attach_meta())
        .args(["completion", "bash"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let script = stdout(&out);
    assert!(script.contains("__complete"), "script: {script}");
    assert!(script.contains("attach-meta"), "script: {script}");
}

#[test]
fn completion_zsh_prints_script() {
    let out = Command::new(attach_meta())
        .args(["completion", "zsh"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let script = stdout(&out);
    assert!(script.contains("compdef"), "script: {script}");
}

#[test]
fn complete_no_subcommand_returns_command_list() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["__complete", "--"]);
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(lines.contains("add"));
    assert!(lines.contains("read"));
    assert!(lines.contains("validate"));
    assert!(lines.contains("completion"));
}

#[test]
fn complete_subcommand_suggests_flags() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    // Without trailing empty string (bare __complete -- add)
    let out = env.run_cmd(&["__complete", "--", "add"]);
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(!lines.contains("--key"), "--key must not appear (it's positional): {lines}");
    assert!(lines.contains("--name"), "missing --name: {lines}");
    assert!(lines.contains("--to"), "missing --to: {lines}");
    assert!(lines.contains("--bus_id"), "missing --bus_id: {lines}");

    // With trailing empty string (what zsh sends for add <TAB>)
    let out = env.run_cmd(&["__complete", "--", "add", ""]);
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(
        !lines.contains("--key"),
        "zsh-style --key must not appear (it's positional): {lines}"
    );
    assert!(
        lines.contains("--name"),
        "zsh-style missing --name: {lines}"
    );
    assert!(lines.contains("--to"), "zsh-style missing --to: {lines}");
    assert!(
        lines.contains("--bus_id"),
        "zsh-style missing --bus_id: {lines}"
    );
}

#[test]
fn complete_partial_flag_filters() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["__complete", "--", "add", "--n"]);
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(lines.contains("--name"), "missing --name: {lines}");
    assert!(
        !lines.contains("--bus_id"),
        "should not contain --bus_id: {lines}"
    );
    assert!(
        !lines.contains("--to"),
        "should not contain --to: {lines}"
    );
}

#[test]
fn complete_excludes_already_used_flags() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["__complete", "--", "add", "--name", "val", "--"]);
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(
        !lines.contains("--name"),
        "should not re-suggest --name: {lines}"
    );
    assert!(lines.contains("--to"), "missing --to: {lines}");
    assert!(lines.contains("--bus_id"), "missing --bus_id: {lines}");
}

#[test]
fn complete_positional_value_via_suggest() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    // Default manifest has completions: [{"arg": "add", "kind": "device-key"}]
    // so positional completion for add should call suggest device-key
    let out = env.run_cmd(&["__complete", "--", "add", ""]);
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(lines.contains("ad7124"), "missing ad7124: {lines}");
    assert!(lines.contains("ad5940"), "missing ad5940: {lines}");
}

#[test]
fn complete_positional_value_filters_by_prefix() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["__complete", "--", "add", "ad71"]);
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(lines.contains("ad7124"), "missing ad7124: {lines}");
    assert!(
        !lines.contains("ad5940"),
        "ad5940 should be filtered: {lines}"
    );
}

#[test]
fn complete_flag_value_via_suggest() {
    let env = TestEnv::new();
    // Manifest with flag-value completion: arg "name" -> kind "device-key"
    env.write_manifest(&format!(
        r#"{{
  "protocol_version": "1.0.0",
  "commands": {{
    "tool-config-get": {{ "argv": ["{bin}", "config-get"] }},
    "tool-config-set": {{ "argv": ["{bin}", "config-set"] }},
    "create-workfile": {{ "argv": ["{bin}", "workfile"] }},
    "list-devices":    {{ "argv": ["{bin}", "devices"] }},
    "add":             {{ "argv": ["{bin}", "add"], "completions": [{{ "arg": "name", "kind": "device-key" }}] }},
    "read":            {{ "argv": ["{bin}", "read"] }},
    "update":          {{ "argv": ["{bin}", "update"] }},
    "delete":          {{ "argv": ["{bin}", "delete"] }},
    "validate":        {{ "argv": ["{bin}", "validate"] }},
    "list-intelligence": {{ "argv": ["{bin}", "list-intelligence"] }},
    "suggest":         {{ "argv": ["{bin}", "suggest"] }}
  }}
}}"#,
        bin = env.tool_path.display()
    ));
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["__complete", "--", "add", "--name", ""]);
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(lines.contains("ad7124"), "missing ad7124: {lines}");
    assert!(lines.contains("ad5940"), "missing ad5940: {lines}");
}

#[test]
fn complete_positional_context_excludes_flag_values() {
    // Regression test: flag values must not be forwarded as positional context.
    // The default manifest has completions: [{"arg": "add", "kind": "device-key"}]
    // Invoking __complete -- add --bus_id myval soc ""  should call:
    //   suggest device-key soc          (positional "soc" only)
    // NOT:
    //   suggest device-key myval soc    (flag value "myval" incorrectly included)
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["__complete", "--", "add", "--bus_id", "myval", "soc", ""]);
    assert!(out.status.success());

    let log = env.invocation_log();
    let suggest_line = log
        .lines()
        .filter(|l| l.contains(" suggest "))
        .last()
        .unwrap_or("");
    assert!(
        suggest_line.ends_with("suggest device-key soc"),
        "expected 'suggest device-key soc', got: {suggest_line}"
    );
}

// ─── Array-flag completions ───

#[test]
fn complete_array_flag_uses_flag_completion_entry() {
    // When the cursor is inside an array flag's value run and the manifest has a
    // completions entry for that flag, the flag's own completion kind is used.
    // add --to soc ""  →  suggest node-key soc
    let env = TestEnv::new();
    env.write_manifest(&format!(
        r#"{{
  "protocol_version": "1.0.0",
  "commands": {{
    "tool-config-get": {{ "argv": ["{bin}", "config-get"] }},
    "tool-config-set": {{ "argv": ["{bin}", "config-set"] }},
    "create-workfile": {{ "argv": ["{bin}", "workfile"] }},
    "list-devices":    {{ "argv": ["{bin}", "devices"] }},
    "add":             {{ "argv": ["{bin}", "add"], "completions": [{{ "arg": "to", "kind": "node-key" }}] }},
    "read":            {{ "argv": ["{bin}", "read"] }},
    "update":          {{ "argv": ["{bin}", "update"] }},
    "delete":          {{ "argv": ["{bin}", "delete"] }},
    "validate":        {{ "argv": ["{bin}", "validate"] }},
    "list-intelligence": {{ "argv": ["{bin}", "list-intelligence"] }},
    "suggest":         {{ "argv": ["{bin}", "suggest"] }}
  }}
}}"#,
        bin = env.tool_path.display()
    ));
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["__complete", "--", "add", "--to", "soc", ""]);
    assert!(out.status.success());

    let log = env.invocation_log();
    let suggest_line = log
        .lines()
        .filter(|l| l.contains(" suggest "))
        .last()
        .unwrap_or("");
    assert!(
        suggest_line.ends_with("suggest node-key soc"),
        "expected 'suggest node-key soc', got: {suggest_line}"
    );
}

#[test]
fn complete_array_flag_grows_context_with_each_value() {
    // Each preceding value inside the array flag run is passed as context.
    // add --to soc i2c ""  →  suggest node-key soc i2c
    let env = TestEnv::new();
    env.write_manifest(&format!(
        r#"{{
  "protocol_version": "1.0.0",
  "commands": {{
    "tool-config-get": {{ "argv": ["{bin}", "config-get"] }},
    "tool-config-set": {{ "argv": ["{bin}", "config-set"] }},
    "create-workfile": {{ "argv": ["{bin}", "workfile"] }},
    "list-devices":    {{ "argv": ["{bin}", "devices"] }},
    "add":             {{ "argv": ["{bin}", "add"], "completions": [{{ "arg": "to", "kind": "node-key" }}] }},
    "read":            {{ "argv": ["{bin}", "read"] }},
    "update":          {{ "argv": ["{bin}", "update"] }},
    "delete":          {{ "argv": ["{bin}", "delete"] }},
    "validate":        {{ "argv": ["{bin}", "validate"] }},
    "list-intelligence": {{ "argv": ["{bin}", "list-intelligence"] }},
    "suggest":         {{ "argv": ["{bin}", "suggest"] }}
  }}
}}"#,
        bin = env.tool_path.display()
    ));
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["__complete", "--", "add", "--to", "soc", "i2c", ""]);
    assert!(out.status.success());

    let log = env.invocation_log();
    let suggest_line = log
        .lines()
        .filter(|l| l.contains(" suggest "))
        .last()
        .unwrap_or("");
    assert!(
        suggest_line.ends_with("suggest node-key soc i2c"),
        "expected 'suggest node-key soc i2c', got: {suggest_line}"
    );
}

#[test]
fn complete_array_flag_no_entry_suggests_flags() {
    // When the cursor is inside an array flag's value run but the manifest has no
    // completions entry for that flag, flag names are suggested instead.
    // Default manifest has no "to" entry → no suggest call, output contains --name.
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["__complete", "--", "add", "--to", "soc", ""]);
    assert!(out.status.success());

    let lines = stdout(&out);
    // Should suggest flags, not invoke suggest with node-key context
    assert!(lines.contains("--name"), "expected --name in flag suggestions: {lines}");

    // No suggest invocation for array-flag context
    let log = env.invocation_log();
    let suggest_calls: Vec<&str> = log
        .lines()
        .filter(|l| l.contains(" suggest "))
        .collect();
    assert!(
        suggest_calls.is_empty(),
        "expected no suggest calls when array flag has no completion entry, got: {suggest_calls:?}"
    );
}

// ─── Move/rename fallback ───

#[test]
fn move_fallback_invokes_read_add_update_delete() {
    let env = TestEnv::new();
    // Manifest WITHOUT move command
    env.write_manifest(&format!(
        r#"{{
  "protocol_version": "1.0.0",
  "commands": {{
    "tool-config-get": {{ "argv": ["{bin}", "config-get"] }},
    "tool-config-set": {{ "argv": ["{bin}", "config-set"] }},
    "create-workfile": {{ "argv": ["{bin}", "workfile"] }},
    "list-devices":    {{ "argv": ["{bin}", "devices"] }},
    "add":             {{ "argv": ["{bin}", "add"] }},
    "read":            {{ "argv": ["{bin}", "read"] }},
    "update":          {{ "argv": ["{bin}", "update"] }},
    "delete":          {{ "argv": ["{bin}", "delete"] }},
    "validate":        {{ "argv": ["{bin}", "validate"] }}
  }}
}}"#,
        bin = env.tool_path.display()
    ));
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["move", "my_node", "--to", "new_parent"]);
    assert!(
        out.status.success(),
        "stdout: {} stderr: {}",
        stdout(&out),
        stderr(&out)
    );

    let log = env.invocation_log();
    // Should see read, then add, then update, then delete --force
    let read_pos = log.find(" read").expect("should call read");
    let add_pos = log.find(" add").expect("should call add");
    let update_pos = log.find(" update").expect("should call update");
    let delete_pos = log.find(" delete").expect("should call delete");
    assert!(read_pos < add_pos, "read should come before add");
    assert!(add_pos < update_pos, "add should come before update");
    assert!(update_pos < delete_pos, "update should come before delete");
    assert!(log.contains("--force"), "delete should use --force");
}

// ─── Array flag forwarding ───

#[test]
fn add_to_array_flag_forwarded_flag_once() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["add", "foo", "--to", "soc", "i2c"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let log = env.invocation_log();
    let add_line = log.lines().find(|l| l.contains(" add ")).unwrap_or("");
    assert!(
        add_line.contains("--to soc i2c"),
        "expected '--to soc i2c' (flag once), got: {add_line}"
    );
    assert!(
        !add_line.contains("--to soc --to"),
        "must not repeat --to flag, got: {add_line}"
    );
}

#[test]
fn move_native_to_array_flag_forwarded_flag_once() {
    let env = TestEnv::new();
    env.write_manifest(&format!(
        r#"{{
  "protocol_version": "1.0.0",
  "commands": {{
    "tool-config-get": {{ "argv": ["{bin}", "config-get"] }},
    "tool-config-set": {{ "argv": ["{bin}", "config-set"] }},
    "create-workfile": {{ "argv": ["{bin}", "workfile"] }},
    "list-devices":    {{ "argv": ["{bin}", "devices"] }},
    "add":             {{ "argv": ["{bin}", "add"] }},
    "read":            {{ "argv": ["{bin}", "read"] }},
    "update":          {{ "argv": ["{bin}", "update"] }},
    "delete":          {{ "argv": ["{bin}", "delete"] }},
    "validate":        {{ "argv": ["{bin}", "validate"] }},
    "move":            {{ "argv": ["{bin}", "move"] }}
  }}
}}"#,
        bin = env.tool_path.display()
    ));
    env.write_tool(
        r#"
    move)
        echo '{"ok":true,"message":"moved","severity":"info"}'
        exit 0
        ;;
"#,
    );
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["move", "my_node", "--to", "soc", "i2c"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let log = env.invocation_log();
    let move_line = log.lines().find(|l| l.contains(" move ")).unwrap_or("");
    assert!(
        move_line.contains("--to soc i2c"),
        "expected '--to soc i2c' (flag once), got: {move_line}"
    );
    assert!(
        !move_line.contains("--to soc --to"),
        "must not repeat --to flag, got: {move_line}"
    );
}

#[test]
fn move_fallback_to_array_flag_forwarded_flag_once() {
    let env = TestEnv::new();
    // Manifest WITHOUT move — triggers fallback that uses push_array_flag in recreate_subtree
    env.write_manifest(&format!(
        r#"{{
  "protocol_version": "1.0.0",
  "commands": {{
    "tool-config-get": {{ "argv": ["{bin}", "config-get"] }},
    "tool-config-set": {{ "argv": ["{bin}", "config-set"] }},
    "create-workfile": {{ "argv": ["{bin}", "workfile"] }},
    "list-devices":    {{ "argv": ["{bin}", "devices"] }},
    "add":             {{ "argv": ["{bin}", "add"] }},
    "read":            {{ "argv": ["{bin}", "read"] }},
    "update":          {{ "argv": ["{bin}", "update"] }},
    "delete":          {{ "argv": ["{bin}", "delete"] }},
    "validate":        {{ "argv": ["{bin}", "validate"] }}
  }}
}}"#,
        bin = env.tool_path.display()
    ));
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["move", "my_node", "--to", "soc", "i2c"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let log = env.invocation_log();
    let add_line = log
        .lines()
        .find(|l| l.contains(" add ") && l.contains("--to"))
        .unwrap_or("");
    assert!(
        add_line.contains("--to soc i2c"),
        "fallback add: expected '--to soc i2c' (flag once), got: {add_line}"
    );
    assert!(
        !add_line.contains("--to soc --to"),
        "fallback add must not repeat --to flag, got: {add_line}"
    );
}

// ─── Exit codes ───

#[test]
fn exit_code_0_for_ok_true() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["list-devices"]);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn exit_code_2_for_input_error() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["add"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unknown_command_exits_2() {
    let out = Command::new(attach_meta())
        .args(["bogus-command"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

// ─── --json mode ───

#[test]
fn json_mode_outputs_json() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["read"]);
    assert!(out.status.success());
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["kind"], "node");
}

#[test]
fn json_mode_error_envelope() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["add"]);
    assert!(!out.status.success());
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["ok"], false);
    assert!(response["error"].is_string());
    assert!(response["category"].is_string());
}
