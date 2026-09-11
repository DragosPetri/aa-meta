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
            .current_dir(self.dir.path())
            .args(["init", self.tool_path.to_str().unwrap(), "--no-interactive"])
            .output()
            .unwrap()
    }

    fn run_cmd(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = Command::new(attach_meta());
        cmd.current_dir(self.dir.path());
        cmd.args(args);
        cmd.output().unwrap()
    }

    fn run_json(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = Command::new(attach_meta());
        cmd.current_dir(self.dir.path());
        cmd.args(["--json"]);
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

struct NoToolEnv {
    dir: tempfile::TempDir,
}

impl NoToolEnv {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        Self { dir }
    }

    fn run_cmd(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = Command::new(attach_meta());
        cmd.current_dir(self.dir.path());
        cmd.args(args);
        cmd.output().unwrap()
    }

    fn run_json(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = Command::new(attach_meta());
        cmd.current_dir(self.dir.path());
        cmd.args(["--json"]);
        cmd.args(args);
        cmd.output().unwrap()
    }

    fn run_cmd_with_path(&self, args: &[&str], extra_path: &Path) -> std::process::Output {
        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{}", extra_path.display(), current_path);
        let mut cmd = Command::new(attach_meta());
        cmd.current_dir(self.dir.path());
        cmd.env("PATH", new_path);
        cmd.args(args);
        cmd.output().unwrap()
    }
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
        .current_dir(env.dir.path())
        .args([
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
    assert!(
        !lines.contains("--key"),
        "--key must not appear (it's positional): {lines}"
    );
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
    assert!(!lines.contains("--to"), "should not contain --to: {lines}");
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
    assert!(
        lines.contains("--name"),
        "expected --name in flag suggestions: {lines}"
    );

    // No suggest invocation for array-flag context
    let log = env.invocation_log();
    let suggest_calls: Vec<&str> = log.lines().filter(|l| l.contains(" suggest ")).collect();
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

// ─── Hash caching (gaps) ───

#[test]
fn hash_unchanged_skips_revalidation() {
    // Write a manifest that would fail meta-schema (missing required "add" command) but store
    // its SHA-256 in the config. Because the hash matches, meta-schema validation is skipped
    // and the command succeeds despite the incomplete manifest.
    let env = TestEnv::new();
    env.write_default_tool(); // must be written before the manifest references its path
    env.write_manifest(&format!(
        r#"{{
        "protocol_version": "1.0.0",
        "commands": {{
            "tool-config-get": {{ "argv": ["{bin}", "config-get"] }},
            "tool-config-set": {{ "argv": ["{bin}", "config-set"] }},
            "create-workfile": {{ "argv": ["{bin}", "workfile"] }},
            "list-devices":    {{ "argv": ["{bin}", "devices"] }},
            "read":            {{ "argv": ["{bin}", "read"] }},
            "update":          {{ "argv": ["{bin}", "update"] }},
            "delete":          {{ "argv": ["{bin}", "delete"] }},
            "validate":        {{ "argv": ["{bin}", "validate"] }}
        }}
    }}"#,
        bin = env.tool_path.display()
    ));
    // write_config_pointing_to_tool computes SHA-256 of the current (bad) manifest and stores it
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["list-devices"]);
    assert!(
        out.status.success(),
        "hash match should skip meta-schema; stderr: {}",
        stderr(&out)
    );
}

#[test]
fn manifest_missing_is_hard_error() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    fs::remove_file(&env.manifest_path).unwrap();

    let out = env.run_cmd(&["list-devices"]);
    assert_eq!(out.status.code(), Some(2));
}

// ─── Version mismatch (gaps) ───

#[test]
fn init_allows_minor_version_mismatch() {
    let env = TestEnv::new();
    env.write_manifest(
        r#"{
        "protocol_version": "1.1.0",
        "commands": {
            "tool-config-get": { "argv": ["t", "cg"] },
            "tool-config-set": { "argv": ["t", "cs"] },
            "create-workfile": { "argv": ["t", "w"] },
            "list-devices":    { "argv": ["t", "d"] },
            "add":             { "argv": ["t", "a"] },
            "read":            { "argv": ["t", "r"] },
            "update":          { "argv": ["t", "u"] },
            "delete":          { "argv": ["t", "del"] },
            "validate":        { "argv": ["t", "v"] }
        }
    }"#,
    );
    env.write_default_tool();

    let out = env.run_init();
    assert!(
        out.status.success(),
        "minor mismatch should be allowed; stderr: {}",
        stderr(&out)
    );
}

#[test]
fn init_allows_patch_version_mismatch() {
    let env = TestEnv::new();
    env.write_manifest(
        r#"{
        "protocol_version": "1.0.99",
        "commands": {
            "tool-config-get": { "argv": ["t", "cg"] },
            "tool-config-set": { "argv": ["t", "cs"] },
            "create-workfile": { "argv": ["t", "w"] },
            "list-devices":    { "argv": ["t", "d"] },
            "add":             { "argv": ["t", "a"] },
            "read":            { "argv": ["t", "r"] },
            "update":          { "argv": ["t", "u"] },
            "delete":          { "argv": ["t", "del"] },
            "validate":        { "argv": ["t", "v"] }
        }
    }"#,
    );
    env.write_default_tool();

    let out = env.run_init();
    assert!(
        out.status.success(),
        "patch mismatch should be allowed; stderr: {}",
        stderr(&out)
    );
}

// ─── Init response fields ───

#[test]
fn init_missing_fields_and_config_complete() {
    // Default fake tool returns workfile with value: null, required: true.
    // Init always calls tool-config-get to compute missing_fields even with --no-interactive.
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();

    let out = Command::new(attach_meta())
        .current_dir(env.dir.path())
        .args([
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

    let missing: Vec<&str> = response["missing_fields"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        missing.contains(&"workfile"),
        "expected 'workfile' in missing_fields, got: {missing:?}"
    );
    assert_eq!(
        response["config_complete"], false,
        "config_complete must be false when required configs are unset"
    );
}

// ─── Config commands ───

#[test]
fn tool_config_get_succeeds() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["tool-config-get"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["ok"], true);
    assert!(response["configs"].is_array());
}

#[test]
fn tool_config_get_with_field_arg() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["tool-config-get", "workfile"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let log = env.invocation_log();
    assert!(
        log.lines()
            .any(|l| l.contains("config-get") && l.contains("workfile")),
        "expected 'config-get workfile' in invocation log, got: {log}"
    );
}

#[test]
fn tool_config_set_missing_value_exits_2() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["tool-config-set", "workfile"]); // missing value arg
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn tool_config_set_succeeds() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["tool-config-set", "workfile", "/path/to/wf"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let log = env.invocation_log();
    assert!(
        log.lines().any(|l| l.contains("config-set")
            && l.contains("workfile")
            && l.contains("/path/to/wf")),
        "expected 'config-set workfile /path/to/wf' in invocation log, got: {log}"
    );
}

// ─── CRUD gaps ───

#[test]
fn add_name_only_succeeds() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["add", "--name", "mydevice"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["ok"], true);
}

#[test]
fn add_key_and_name_succeeds() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["add", "mykey", "--name", "mydevice"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let log = env.invocation_log();
    let add_line = log.lines().find(|l| l.contains(" add ")).unwrap_or("");
    assert!(add_line.contains("mykey"), "expected mykey: {add_line}");
    assert!(add_line.contains("--name"), "expected --name: {add_line}");
    assert!(
        add_line.contains("mydevice"),
        "expected mydevice: {add_line}"
    );
}

#[test]
fn add_response_includes_path() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["add", "my_device"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    let path = response["path"]
        .as_array()
        .expect("path should be an array");
    assert!(!path.is_empty(), "path should be non-empty");
}

#[test]
fn delete_no_args_returns_preview() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["delete"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert!(
        response.get("node_count").is_some(),
        "expected DeletePreview with node_count: {response}"
    );
}

#[test]
fn delete_force_no_args_deletes_all() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["delete", "--force"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["ok"], true);
    assert!(
        response.get("node_count").is_none(),
        "expected CommonResponse, not DeletePreview: {response}"
    );
}

// ─── Unknown flag forwarding ───

#[test]
fn unknown_flag_treated_as_string_and_forwarded() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["read", "--undeclared-flag", "somevalue"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let log = env.invocation_log();
    assert!(
        log.contains("--undeclared-flag") && log.contains("somevalue"),
        "expected unknown flag forwarded to subtool: {log}"
    );
}

// ─── Rename ───

#[test]
fn rename_native_invokes_subtool() {
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
    "rename":          {{ "argv": ["{bin}", "rename"] }}
  }}
}}"#,
        bin = env.tool_path.display()
    ));
    env.write_tool(
        r#"
    rename)
        echo '{"ok":true,"message":"renamed","severity":"info"}'
        exit 0
        ;;
"#,
    );
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["rename", "my_node", "--to", "new_name"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let log = env.invocation_log();
    let rename_line = log.lines().find(|l| l.contains(" rename ")).unwrap_or("");
    assert!(
        rename_line.contains("my_node"),
        "expected my_node: {rename_line}"
    );
    assert!(rename_line.contains("--to"), "expected --to: {rename_line}");
    assert!(
        rename_line.contains("new_name"),
        "expected new_name: {rename_line}"
    );
}

#[test]
fn rename_fallback_node_read_add_update_delete() {
    let env = TestEnv::new();
    // Default manifest has no rename — fallback activates.
    // read "my_node" returns a Node, triggering node rename fallback.
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["rename", "my_node", "--to", "new_name"]);
    assert!(
        out.status.success(),
        "stdout: {} stderr: {}",
        stdout(&out),
        stderr(&out)
    );

    let log = env.invocation_log();
    let read_pos = log.find(" read").expect("fallback should call read");
    let add_pos = log.find(" add").expect("fallback should call add");
    let update_pos = log.find(" update").expect("fallback should call update");
    let delete_pos = log.find(" delete").expect("fallback should call delete");
    assert!(read_pos < add_pos, "read before add");
    assert!(add_pos < update_pos, "add before update");
    assert!(update_pos < delete_pos, "update before delete");
    assert!(
        log.contains("--force"),
        "node rename delete must use --force"
    );
}

#[test]
fn rename_fallback_property_read_update_delete() {
    let env = TestEnv::new();
    // read "my_node temperature" returns a Property, triggering property rename fallback.
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["rename", "my_node", "temperature", "--to", "temp_c"]);
    assert!(
        out.status.success(),
        "stdout: {} stderr: {}",
        stdout(&out),
        stderr(&out)
    );

    let log = env.invocation_log();
    let read_pos = log.find(" read").expect("fallback should call read");
    let update_pos = log.find(" update").expect("fallback should call update");
    let delete_pos = log.find(" delete").expect("fallback should call delete");
    assert!(read_pos < update_pos, "read before update");
    assert!(update_pos < delete_pos, "update before delete");

    // Property delete must NOT use --force (leaf/property deletes directly)
    let delete_line = log.lines().rfind(|l| l.contains(" delete")).unwrap_or("");
    assert!(
        !delete_line.contains("--force"),
        "property rename delete must not use --force: {delete_line}"
    );
}

// ─── Alias ───

fn write_alias_manifest_and_tool(env: &TestEnv) {
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
    "alias":           {{ "argv": ["{bin}", "alias"] }}
  }}
}}"#,
        bin = env.tool_path.display()
    ));
    env.write_tool(
        r#"
    alias)
        echo '{"ok":true,"message":"alias ok","severity":"info"}'
        exit 0
        ;;
"#,
    );
}

#[test]
fn alias_with_invokes_subtool() {
    let env = TestEnv::new();
    write_alias_manifest_and_tool(&env);
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["alias", "my_node", "--with", "my_alias"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let log = env.invocation_log();
    let alias_line = log.lines().find(|l| l.contains(" alias ")).unwrap_or("");
    assert!(
        alias_line.contains("my_node"),
        "expected my_node: {alias_line}"
    );
    assert!(
        alias_line.contains("--with"),
        "expected --with: {alias_line}"
    );
    assert!(
        alias_line.contains("my_alias"),
        "expected my_alias: {alias_line}"
    );
}

#[test]
fn alias_remove_invokes_subtool() {
    let env = TestEnv::new();
    write_alias_manifest_and_tool(&env);
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["alias", "my_node", "--remove", "my_alias"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let log = env.invocation_log();
    let alias_line = log.lines().find(|l| l.contains(" alias ")).unwrap_or("");
    assert!(
        alias_line.contains("my_node"),
        "expected my_node: {alias_line}"
    );
    assert!(
        alias_line.contains("--remove"),
        "expected --remove: {alias_line}"
    );
    assert!(
        alias_line.contains("my_alias"),
        "expected my_alias: {alias_line}"
    );
}

#[test]
fn alias_not_in_manifest_exits_2() {
    let env = TestEnv::new();
    env.write_default_manifest(); // no alias command
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["alias", "my_node", "--with", "x"]);
    assert_eq!(out.status.code(), Some(2));
}

// ─── __complete gaps ───

#[test]
fn complete_partial_subcommand_prefix_filter() {
    let env = TestEnv::new();
    // Include optional "rename" so we can verify that optional commands present in
    // the manifest DO participate in prefix matching.
    let bin = env.tool_path.display();
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
    "rename":          {{ "argv": ["{bin}", "rename"] }}
  }}
}}"#
    ));
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["__complete", "--", "re"]);
    assert!(out.status.success());
    let lines = stdout(&out);
    // Both required "read" and optional "rename" declared in the manifest must appear.
    assert!(lines.contains("read"), "expected 'read': {lines}");
    assert!(
        lines.contains("rename"),
        "expected 'rename' (declared in manifest): {lines}"
    );
    // Commands that don't match the prefix must not appear.
    assert!(!lines.contains("add"), "must not contain 'add': {lines}");
    assert!(
        !lines.contains("validate"),
        "must not contain 'validate': {lines}"
    );
    // Optional commands absent from this manifest must not appear.
    assert!(
        !lines.contains("move"),
        "must not contain 'move' (not in manifest): {lines}"
    );
}

#[test]
fn complete_suggest_kind_completion() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    // Completing the kind argument for suggest — should call list-intelligence
    let out = env.run_cmd(&["__complete", "--", "suggest", ""]);
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(lines.contains("device-key"), "expected device-key: {lines}");
    assert!(lines.contains("node-key"), "expected node-key: {lines}");

    let log = env.invocation_log();
    assert!(
        log.contains("list-intelligence"),
        "expected list-intelligence call in log: {log}"
    );
}

#[test]
fn complete_array_flag_suppresses_flag_names() {
    // When the cursor is inside an array flag's value run and the manifest has a
    // completions entry for that flag, step 3 (flag names) is suppressed so that
    // flag names and value suggestions do not mix.
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

    // Inside --to array flag (has completion entry), partial is "" — step 3 is suppressed
    let out = env.run_cmd(&["__complete", "--", "add", "--to", "soc", ""]);
    assert!(out.status.success());
    let lines = stdout(&out);

    // Step 4 runs suggest node-key → ad7124, ad5940
    assert!(lines.contains("ad7124"), "expected ad7124: {lines}");
    assert!(lines.contains("ad5940"), "expected ad5940: {lines}");

    // Step 3 suppressed — flag names must not appear alongside value completions
    assert!(
        !lines.contains("--name"),
        "flag names must be suppressed inside array flag with completion entry: {lines}"
    );
}

// ─── suggest / list-intelligence direct ───

#[test]
fn list_intelligence_direct_invocation() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["list-intelligence"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["ok"], true);
    assert!(response["intelligence"].is_array());
}

#[test]
fn suggest_direct_invocation() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["suggest", "device-key"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["ok"], true);
    assert!(response["suggestions"].is_array());
}

#[test]
fn suggest_unadvertised_kind_exits_2() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["suggest", "totally-unknown-kind"]);
    assert_eq!(out.status.code(), Some(2));
}

// ─── move source path guard ───

#[test]
fn move_requires_source_path() {
    let env = TestEnv::new();
    env.write_default_manifest(); // no native move — fallback path
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["move", "--to", "new_parent"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "move without source path must exit 2; stderr: {}",
        stderr(&out)
    );
}

// ─── meta-intelligence ───

#[test]
fn meta_list_intelligence_no_tool() {
    let env = NoToolEnv::new();
    let out = env.run_json(&["list-intelligence"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["ok"], true);
    let intelligence = response["intelligence"].as_array().unwrap();
    assert!(
        intelligence.iter().any(|i| i["kind"] == "attachable"),
        "expected 'attachable' in intelligence: {intelligence:?}"
    );
}

#[test]
fn meta_list_intelligence_no_tool_human_output() {
    let env = NoToolEnv::new();
    let out = env.run_cmd(&["list-intelligence"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("attachable"),
        "expected 'attachable' in human output: {err}"
    );
}

#[test]
fn meta_suggest_attachable_no_tool() {
    let env = NoToolEnv::new();
    let bin_dir = tempfile::tempdir().unwrap();

    // Create fake attach-* binaries
    let fake1 = bin_dir.path().join("attach-foo");
    let fake2 = bin_dir.path().join("attach-bar");
    let not_attach = bin_dir.path().join("other-tool");
    fs::write(&fake1, "#!/bin/sh\n").unwrap();
    fs::write(&fake2, "#!/bin/sh\n").unwrap();
    fs::write(&not_attach, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&fake1, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(&fake2, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(&not_attach, fs::Permissions::from_mode(0o755)).unwrap();

    let out = env.run_cmd_with_path(&["--json", "suggest", "attachable"], bin_dir.path());
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(response["ok"], true);
    let suggestions = response["suggestions"].as_array().unwrap();
    let values: Vec<&str> = suggestions
        .iter()
        .map(|s| s["value"].as_str().unwrap())
        .collect();
    assert!(
        values.contains(&"attach-bar"),
        "expected attach-bar: {values:?}"
    );
    assert!(
        values.contains(&"attach-foo"),
        "expected attach-foo: {values:?}"
    );
    assert!(
        !values.contains(&"other-tool"),
        "should not contain other-tool: {values:?}"
    );
    assert!(
        !values.contains(&"attach-meta"),
        "should not contain attach-meta: {values:?}"
    );
}

#[test]
fn meta_suggest_unknown_kind_no_tool_exits_2() {
    let env = NoToolEnv::new();
    let out = env.run_cmd(&["suggest", "totally-unknown"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "suggest unknown kind without tool should exit 2; stderr: {}",
        stderr(&out)
    );
}

#[test]
fn meta_suggest_tool_kind_no_tool_exits_2() {
    let env = NoToolEnv::new();
    let out = env.run_cmd(&["suggest", "device-key"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "suggest tool-only kind without tool should exit 2; stderr: {}",
        stderr(&out)
    );
}

#[test]
fn meta_intelligence_merges_with_tool() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["list-intelligence"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    let intelligence = response["intelligence"].as_array().unwrap();
    let kinds: Vec<&str> = intelligence
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert!(
        kinds.contains(&"attachable"),
        "expected 'attachable' from meta: {kinds:?}"
    );
    assert!(
        kinds.contains(&"device-key"),
        "expected 'device-key' from tool: {kinds:?}"
    );
    assert!(
        kinds.contains(&"node-key"),
        "expected 'node-key' from tool: {kinds:?}"
    );
}

#[test]
fn meta_suggest_attachable_with_tool_uses_meta() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let bin_dir = tempfile::tempdir().unwrap();
    let fake = bin_dir.path().join("attach-test");
    fs::write(&fake, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();

    let current_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.path().display(), current_path);
    let out = Command::new(attach_meta())
        .current_dir(env.dir.path())
        .env("PATH", new_path)
        .args(["--json", "suggest", "attachable"])
        .output()
        .unwrap();

    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    let suggestions = response["suggestions"].as_array().unwrap();
    let values: Vec<&str> = suggestions
        .iter()
        .map(|s| s["value"].as_str().unwrap())
        .collect();
    assert!(
        values.contains(&"attach-test"),
        "expected attach-test in meta suggestions: {values:?}"
    );

    // Should NOT have called the tool's suggest — check invocation log
    let log = env.invocation_log();
    assert!(
        !log.contains(" suggest "),
        "suggest attachable should be handled by meta, not tool: {log}"
    );
}

#[test]
fn meta_intelligence_collision_warns() {
    let env = TestEnv::new();
    // Tool that returns "attachable" as one of its intelligence kinds (collision)
    env.write_tool(
        r#"
    list-intelligence)
        echo '{"ok":true,"message":"intelligence","severity":"info","intelligence":[{"kind":"attachable","args":[]},{"kind":"device-key","args":[]}]}'
        exit 0
        ;;
    suggest)
        shift
        echo '{"ok":true,"message":"suggestions","severity":"info","suggestions":[{"value":"tool-value"}]}'
        exit 0
        ;;"#,
    );

    // Manifest with list-intelligence and suggest but NO default handlers
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
    "list-intelligence": {{ "argv": ["{bin}", "list-intelligence"] }},
    "suggest":         {{ "argv": ["{bin}", "suggest"] }}
  }}
}}"#,
        bin = env.tool_path.display()
    ));
    env.write_config_pointing_to_tool();

    let out = env.run_json(&["list-intelligence"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    // Check warning on stderr
    let err = stderr(&out);
    assert!(
        err.contains("warning") && err.contains("attachable"),
        "expected collision warning about 'attachable' on stderr: {err}"
    );

    // Meta version should be preferred
    let response: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    let intelligence = response["intelligence"].as_array().unwrap();
    let kinds: Vec<&str> = intelligence
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert!(
        kinds.contains(&"attachable"),
        "meta 'attachable' should be present: {kinds:?}"
    );
    assert!(
        kinds.contains(&"device-key"),
        "tool 'device-key' should be present: {kinds:?}"
    );

    // Only one "attachable" (meta wins, tool's is dropped)
    let attachable_count = kinds.iter().filter(|k| **k == "attachable").count();
    assert_eq!(
        attachable_count, 1,
        "should have exactly one 'attachable', not duplicated"
    );
}

#[test]
fn complete_init_lists_attachables() {
    let env = NoToolEnv::new();
    let bin_dir = tempfile::tempdir().unwrap();

    let fake = bin_dir.path().join("attach-example");
    fs::write(&fake, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();

    let out = env.run_cmd_with_path(&["__complete", "--", "init", ""], bin_dir.path());
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(
        lines.contains("attach-example"),
        "expected attach-example in init completions: {lines}"
    );
}

#[test]
fn complete_init_filters_by_prefix() {
    let env = NoToolEnv::new();
    let bin_dir = tempfile::tempdir().unwrap();

    let fake1 = bin_dir.path().join("attach-alpha");
    let fake2 = bin_dir.path().join("attach-beta");
    fs::write(&fake1, "#!/bin/sh\n").unwrap();
    fs::write(&fake2, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&fake1, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(&fake2, fs::Permissions::from_mode(0o755)).unwrap();

    let out = env.run_cmd_with_path(&["__complete", "--", "init", "attach-a"], bin_dir.path());
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(
        lines.contains("attach-alpha"),
        "expected attach-alpha: {lines}"
    );
    assert!(
        !lines.contains("attach-beta"),
        "attach-beta should be filtered out: {lines}"
    );
}

#[test]
fn complete_suggest_includes_meta_kinds() {
    let env = TestEnv::new();
    env.write_default_manifest();
    env.write_default_tool();
    env.write_config_pointing_to_tool();

    let out = env.run_cmd(&["__complete", "--", "suggest", ""]);
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(
        lines.contains("attachable"),
        "expected 'attachable' in suggest kind completions: {lines}"
    );
    assert!(
        lines.contains("device-key"),
        "expected 'device-key' in suggest kind completions: {lines}"
    );
}

#[test]
fn complete_no_subcommand_includes_init() {
    let env = NoToolEnv::new();
    let out = env.run_cmd(&["__complete", "--"]);
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(
        lines.contains("init"),
        "expected 'init' in command list: {lines}"
    );
}

#[test]
fn complete_no_tool_excludes_tool_commands() {
    let env = NoToolEnv::new();
    let out = env.run_cmd(&["__complete", "--"]);
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(lines.contains("init"), "expected 'init': {lines}");
    assert!(
        lines.contains("completion"),
        "expected 'completion': {lines}"
    );
    for cmd in &[
        "add",
        "read",
        "update",
        "delete",
        "validate",
        "tool-config-get",
        "list-devices",
    ] {
        assert!(
            !lines.contains(cmd),
            "must not suggest '{cmd}' without a tool: {lines}"
        );
    }
}

#[test]
fn complete_no_tool_partial_excludes_tool_commands() {
    let env = NoToolEnv::new();
    // "ad" would match "add" — must not be suggested without a tool
    let out = env.run_cmd(&["__complete", "--", "ad"]);
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(
        !lines.contains("add"),
        "must not suggest 'add' without a tool: {lines}"
    );
    // "co" matches "completion" — must still appear
    let out = env.run_cmd(&["__complete", "--", "co"]);
    assert!(out.status.success());
    let lines = stdout(&out);
    assert!(
        lines.contains("completion"),
        "expected 'completion' for partial 'co': {lines}"
    );
}
