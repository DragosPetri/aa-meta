use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandName {
    ToolConfigGet,
    ToolConfigSet,
    CreateWorkfile,
    ListDevices,
    Add,
    Read,
    Update,
    Delete,
    Move,
    Rename,
    Alias,
    Validate,
    Generate,
    Build,
    Deploy,
    ListIntelligence,
    Suggest,
}

impl CommandName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ToolConfigGet => "tool-config-get",
            Self::ToolConfigSet => "tool-config-set",
            Self::CreateWorkfile => "create-workfile",
            Self::ListDevices => "list-devices",
            Self::Add => "add",
            Self::Read => "read",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Move => "move",
            Self::Rename => "rename",
            Self::Alias => "alias",
            Self::Validate => "validate",
            Self::Generate => "generate",
            Self::Build => "build",
            Self::Deploy => "deploy",
            Self::ListIntelligence => "list-intelligence",
            Self::Suggest => "suggest",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "tool-config-get" => Some(Self::ToolConfigGet),
            "tool-config-set" => Some(Self::ToolConfigSet),
            "create-workfile" => Some(Self::CreateWorkfile),
            "list-devices" => Some(Self::ListDevices),
            "add" => Some(Self::Add),
            "read" => Some(Self::Read),
            "update" => Some(Self::Update),
            "delete" => Some(Self::Delete),
            "move" => Some(Self::Move),
            "rename" => Some(Self::Rename),
            "alias" => Some(Self::Alias),
            "validate" => Some(Self::Validate),
            "generate" => Some(Self::Generate),
            "build" => Some(Self::Build),
            "deploy" => Some(Self::Deploy),
            "list-intelligence" => Some(Self::ListIntelligence),
            "suggest" => Some(Self::Suggest),
            _ => None,
        }
    }

    #[cfg(test)]
    pub const REQUIRED: &[CommandName] = &[
        Self::ToolConfigGet,
        Self::ToolConfigSet,
        Self::CreateWorkfile,
        Self::ListDevices,
        Self::Add,
        Self::Read,
        Self::Update,
        Self::Delete,
        Self::Validate,
    ];

    pub const ALL: &[CommandName] = &[
        Self::ToolConfigGet,
        Self::ToolConfigSet,
        Self::CreateWorkfile,
        Self::ListDevices,
        Self::Add,
        Self::Read,
        Self::Update,
        Self::Delete,
        Self::Move,
        Self::Rename,
        Self::Alias,
        Self::Validate,
        Self::Generate,
        Self::Build,
        Self::Deploy,
        Self::ListIntelligence,
        Self::Suggest,
    ];
}

impl std::fmt::Display for CommandName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompletionHint {
    pub arg: String,
    pub kind: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CommandMapping {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completions: Option<Vec<CompletionHint>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub protocol_version: String,
    pub commands: HashMap<String, CommandMapping>,
}

impl Manifest {
    pub fn get_command(&self, name: CommandName) -> Option<&CommandMapping> {
        self.commands.get(name.as_str())
    }
}

pub fn parse_manifest(json: &str) -> anyhow::Result<Manifest> {
    serde_json::from_str(json).map_err(|e| anyhow::anyhow!("failed to parse manifest: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    static MANIFEST_SCHEMA: &str = include_str!("../../docs/schemas/manifest.schema.json");

    #[test]
    fn command_name_roundtrip() {
        for cmd in CommandName::ALL {
            let s = cmd.as_str();
            assert_eq!(
                CommandName::from_str(s),
                Some(*cmd),
                "roundtrip failed for {s}"
            );
        }
    }

    #[test]
    fn parse_minimal_manifest() {
        let json = r#"{
            "protocol_version": "1.0.0",
            "commands": {
                "add": { "argv": ["tool", "add"] },
                "read": { "argv": ["tool", "read"] },
                "update": { "argv": ["tool", "update"] },
                "delete": { "argv": ["tool", "delete"] },
                "validate": { "argv": ["tool", "validate"] },
                "tool-config-get": { "argv": ["tool", "config-get"] },
                "tool-config-set": { "argv": ["tool", "config-set"] },
                "create-workfile": { "argv": ["tool", "workfile"] },
                "list-devices": { "argv": ["tool", "devices"] }
            }
        }"#;
        let m = parse_manifest(json).unwrap();
        assert_eq!(m.protocol_version, "1.0.0");
        assert!(m.get_command(CommandName::Add).is_some());
        assert!(m.get_command(CommandName::Move).is_none());
    }

    #[test]
    fn parse_manifest_with_args_and_completions() {
        let json = r#"{
            "protocol_version": "1.0.0",
            "commands": {
                "add": {
                    "argv": ["tool", "add"],
                    "args": {
                        "properties": {
                            "bus_id": { "type": "string" }
                        }
                    },
                    "timeout_ms": 5000,
                    "completions": [
                        { "arg": "add", "kind": "device-key" },
                        { "arg": "bus_id", "kind": "bus-id" }
                    ]
                },
                "read": { "argv": ["tool", "read"] },
                "update": { "argv": ["tool", "update"] },
                "delete": { "argv": ["tool", "delete"] },
                "validate": { "argv": ["tool", "validate"] },
                "tool-config-get": { "argv": ["tool", "config-get"] },
                "tool-config-set": { "argv": ["tool", "config-set"] },
                "create-workfile": { "argv": ["tool", "workfile"] },
                "list-devices": { "argv": ["tool", "devices"] }
            }
        }"#;
        let m = parse_manifest(json).unwrap();
        let add = m.get_command(CommandName::Add).unwrap();
        assert_eq!(add.timeout_ms, Some(5000));
        assert!(add.args.is_some());
        let comps = add.completions.as_ref().unwrap();
        assert_eq!(comps.len(), 2);
        assert_eq!(comps[0].kind, "device-key");
    }

    // ── Schema sync checks ──

    fn manifest_schema() -> serde_json::Value {
        serde_json::from_str(MANIFEST_SCHEMA).unwrap()
    }

    fn manifest_validator(pointer: Option<&str>) -> jsonschema::Validator {
        let root = manifest_schema();
        let subschema = match pointer {
            Some(p) => root
                .pointer(p)
                .unwrap_or_else(|| panic!("no schema at pointer '{p}'"))
                .clone(),
            None => serde_json::json!({ "$ref": "attach-meta/manifest" }),
        };
        jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .with_resource(
                "attach-meta/manifest",
                jsonschema::Resource::from_contents(root).unwrap(),
            )
            .build(&subschema)
            .unwrap_or_else(|e| panic!("failed to compile manifest schema: {e}"))
    }

    fn assert_manifest_valid(pointer: Option<&str>, instance: &impl Serialize) {
        let validator = manifest_validator(pointer);
        let value = serde_json::to_value(instance).unwrap();
        let errors: Vec<_> = validator
            .iter_errors(&value)
            .map(|e| format!("  - {e}"))
            .collect();
        assert!(
            errors.is_empty(),
            "Manifest schema validation failed{}:\n{}",
            pointer.map(|p| format!(" at {p}")).unwrap_or_default(),
            errors.join("\n"),
        );
    }

    #[test]
    fn schema_sync_manifest_top_level() {
        assert_manifest_valid(
            None,
            &Manifest {
                protocol_version: "1.0.0".into(),
                commands: CommandName::REQUIRED
                    .iter()
                    .map(|c| {
                        (
                            c.as_str().to_string(),
                            CommandMapping {
                                description: None,
                                argv: vec!["t".into()],
                                args: None,
                                timeout_ms: None,
                                completions: None,
                            },
                        )
                    })
                    .collect(),
            },
        );
    }

    #[test]
    fn schema_sync_command_mapping() {
        assert_manifest_valid(
            Some("/$defs/CommandMapping"),
            &CommandMapping {
                description: Some("test description".into()),
                argv: vec!["t".into()],
                args: Some(serde_json::json!({})),
                timeout_ms: Some(1),
                completions: Some(vec![CompletionHint {
                    arg: "a".into(),
                    kind: "k".into(),
                }]),
            },
        );
    }

    #[test]
    fn schema_sync_completion_hint() {
        assert_manifest_valid(
            Some("/$defs/CommandMapping/properties/completions/items"),
            &CompletionHint {
                arg: "a".into(),
                kind: "k".into(),
            },
        );
    }

    #[test]
    fn schema_sync_command_names_match_property_names_enum() {
        let schema = manifest_schema();
        let allowed: std::collections::BTreeSet<String> =
            schema["properties"]["commands"]["propertyNames"]["enum"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();

        let rust_all: std::collections::BTreeSet<String> = CommandName::ALL
            .iter()
            .map(|c| c.as_str().to_string())
            .collect();

        let in_schema_not_rust: Vec<_> = allowed.difference(&rust_all).collect();
        let in_rust_not_schema: Vec<_> = rust_all.difference(&allowed).collect();
        assert!(
            in_schema_not_rust.is_empty() && in_rust_not_schema.is_empty(),
            "CommandName::ALL vs schema propertyNames.enum drift:\n  \
             in schema not in ALL: {in_schema_not_rust:?}\n  \
             in ALL not in schema: {in_rust_not_schema:?}"
        );
    }

    #[test]
    fn schema_sync_required_commands_match() {
        let schema = manifest_schema();
        let required: std::collections::BTreeSet<String> =
            schema["properties"]["commands"]["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();

        let rust_required: std::collections::BTreeSet<String> = CommandName::REQUIRED
            .iter()
            .map(|c| c.as_str().to_string())
            .collect();

        let in_schema_not_rust: Vec<_> = required.difference(&rust_required).collect();
        let in_rust_not_schema: Vec<_> = rust_required.difference(&required).collect();
        assert!(
            in_schema_not_rust.is_empty() && in_rust_not_schema.is_empty(),
            "CommandName::REQUIRED vs schema commands.required drift:\n  \
             in schema not in REQUIRED: {in_schema_not_rust:?}\n  \
             in REQUIRED not in schema: {in_rust_not_schema:?}"
        );
    }
}
