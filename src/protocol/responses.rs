use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommonResponse {
    pub ok: bool,
    pub message: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => f.write_str("info"),
            Self::Warn => f.write_str("warn"),
            Self::Error => f.write_str("error"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InitResponse {
    pub ok: bool,
    pub message: String,
    pub severity: Severity,
    pub config_complete: bool,
    pub missing_fields: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub field_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub description: String,
    #[serde(rename = "type")]
    pub config_type: ConfigType,
    pub required: bool,
    pub default: serde_json::Value,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ConfigType {
    Scalar(ScalarType),
    Enum(ConfigEnum),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScalarType {
    Numeric,
    String,
    Bool,
    Path,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigEnum {
    pub options: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolConfigResponse {
    pub ok: bool,
    pub message: String,
    pub severity: Severity,
    pub configs: Vec<Config>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateWorkfileResponse {
    pub ok: bool,
    pub message: String,
    pub severity: Severity,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Device {
    pub tag: String,
    pub key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListDevicesResponse {
    pub ok: bool,
    pub message: String,
    pub severity: Severity,
    pub devices: Vec<Device>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AddResponse {
    pub ok: bool,
    pub message: String,
    pub severity: Severity,
    pub key: String,
    pub path: Vec<String>,
}

// TYPES — discriminated union on "kind"
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum Types {
    #[serde(rename = "number")]
    Number { subtype: NumberSubtype },
    #[serde(rename = "string")]
    String,
    #[serde(rename = "bool")]
    Bool,
    #[serde(rename = "enum")]
    Enum { options: Vec<EnumOption> },
    #[serde(rename = "array")]
    Array { items: Box<Types> },
    #[serde(rename = "tuple")]
    Tuple { items: Vec<Types> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NumberSubtype {
    Int,
    Float,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnumOption {
    pub value: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_string: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Property {
    pub kind: String, // always "property"
    pub key: String,
    #[serde(rename = "type")]
    pub prop_type: Types,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Node {
    pub kind: String, // always "node"
    pub key: String,
    pub properties: Vec<Property>,
    pub children: Vec<Node>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ReadResponse {
    Node(Node),
    Property(Property),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeletePreview {
    pub ok: bool,
    pub message: String,
    pub severity: Severity,
    pub node_count: u64,
    pub property_count: u64,
    pub paths: Vec<Vec<String>>,
}

#[cfg(test)]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DeleteResponse {
    Preview(DeletePreview),
    Common(CommonResponse),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ValidationError {
    pub kind: String, // "generic"
    pub path: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ValidationResponse {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IntelligenceArg {
    pub name: String,
    pub description: String,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Intelligence {
    pub kind: String,
    pub args: Vec<IntelligenceArg>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListIntelligenceResponse {
    pub ok: bool,
    pub message: String,
    pub severity: Severity,
    pub intelligence: Vec<Intelligence>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Suggestion {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_string: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SuggestResponse {
    pub ok: bool,
    pub message: String,
    pub severity: Severity,
    pub suggestions: Vec<Suggestion>,
}

#[cfg(test)]
mod tests {
    use super::*;

    static RESPONSES_SCHEMA: &str = include_str!("../../docs/schemas/responses.schema.json");

    #[test]
    fn common_response_roundtrip() {
        let json = r#"{"ok":true,"message":"done","severity":"info"}"#;
        let r: CommonResponse = serde_json::from_str(json).unwrap();
        assert!(r.ok);
        assert_eq!(r.severity, Severity::Info);
        let back = serde_json::to_string(&r).unwrap();
        assert!(back.contains("\"ok\":true"));
    }

    #[test]
    fn types_number() {
        let json = r#"{"kind":"number","subtype":"int"}"#;
        let t: Types = serde_json::from_str(json).unwrap();
        assert!(matches!(
            t,
            Types::Number {
                subtype: NumberSubtype::Int
            }
        ));
    }

    #[test]
    fn types_array_nested() {
        let json = r#"{"kind":"array","items":{"kind":"string"}}"#;
        let t: Types = serde_json::from_str(json).unwrap();
        assert!(matches!(t, Types::Array { .. }));
    }

    #[test]
    fn node_with_children() {
        let json = r#"{
            "kind": "node",
            "key": "root",
            "properties": [],
            "children": [{
                "kind": "node",
                "key": "child",
                "properties": [{"kind":"property","key":"p","type":{"kind":"string"},"value":"v"}],
                "children": []
            }]
        }"#;
        let n: Node = serde_json::from_str(json).unwrap();
        assert_eq!(n.key, "root");
        assert_eq!(n.children.len(), 1);
        assert_eq!(n.children[0].properties.len(), 1);
    }

    #[test]
    fn delete_response_preview() {
        let json = r#"{"ok":true,"message":"preview","severity":"info","node_count":3,"property_count":5,"paths":[["a","b"],["a","c"]]}"#;
        let r: DeleteResponse = serde_json::from_str(json).unwrap();
        assert!(matches!(r, DeleteResponse::Preview(_)));
    }

    #[test]
    fn delete_response_common() {
        let json = r#"{"ok":true,"message":"deleted","severity":"info"}"#;
        let r: DeleteResponse = serde_json::from_str(json).unwrap();
        assert!(matches!(r, DeleteResponse::Common(_)));
    }

    #[test]
    fn read_response_node_vs_property() {
        let node_json = r#"{"kind":"node","key":"r","properties":[],"children":[]}"#;
        let r: ReadResponse = serde_json::from_str(node_json).unwrap();
        assert!(matches!(r, ReadResponse::Node(_)));

        let prop_json = r#"{"kind":"property","key":"p","type":{"kind":"string"},"value":"v"}"#;
        let r: ReadResponse = serde_json::from_str(prop_json).unwrap();
        assert!(matches!(r, ReadResponse::Property(_)));
    }

    #[test]
    fn validation_response_roundtrip() {
        let json =
            r#"{"errors":[{"kind":"generic","path":["a","b"],"message":"bad"}],"warnings":[]}"#;
        let r: ValidationResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.errors.len(), 1);
        assert_eq!(r.errors[0].path, vec!["a", "b"]);
    }

    #[test]
    fn config_type_scalar_and_enum() {
        let scalar = r#""string""#;
        let t: ConfigType = serde_json::from_str(scalar).unwrap();
        assert!(matches!(t, ConfigType::Scalar(ScalarType::String)));

        let enum_t = r#"{"options":["a","b"]}"#;
        let t: ConfigType = serde_json::from_str(enum_t).unwrap();
        assert!(matches!(t, ConfigType::Enum(_)));
    }

    // ── Schema sync checks ──
    //
    // These tests detect drift between the Rust structs and
    // docs/schemas/responses.schema.json. Each one builds a representative
    // instance, serializes it, and validates against the matching $def.
    // Uses the same jsonschema crate the rest of the codebase depends on,
    // so type mismatches and extra/missing fields are both caught.

    fn def_validator(def_name: &str) -> jsonschema::Validator {
        let root: serde_json::Value = serde_json::from_str(RESPONSES_SCHEMA).unwrap();
        let ref_schema = serde_json::json!({
            "$ref": format!("attach-meta/responses#/$defs/{def_name}")
        });
        jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .with_resource(
                "attach-meta/responses",
                jsonschema::Resource::from_contents(root).unwrap(),
            )
            .build(&ref_schema)
            .unwrap_or_else(|e| panic!("failed to compile schema for '{def_name}': {e}"))
    }

    fn schema_union_branch_count(def_name: &str) -> usize {
        let root: serde_json::Value = serde_json::from_str(RESPONSES_SCHEMA).unwrap();
        let def = root
            .pointer(&format!("/$defs/{def_name}"))
            .unwrap_or_else(|| panic!("no $def named '{def_name}'"));
        def.get("anyOf")
            .or_else(|| def.get("oneOf"))
            .and_then(|v| v.as_array())
            .unwrap_or_else(|| panic!("'{def_name}' has no anyOf/oneOf"))
            .len()
    }

    macro_rules! assert_union_schema_valid {
        ($def_name:expr, $($instance:expr),+ $(,)?) => {{
            let instances = [$( serde_json::to_value(&$instance).unwrap() ),+];
            assert_eq!(
                schema_union_branch_count($def_name),
                instances.len(),
                "'{0}' schema has {1} branches but test supplies {2} variants",
                $def_name,
                schema_union_branch_count($def_name),
                instances.len(),
            );
            let validator = def_validator($def_name);
            for value in &instances {
                let errors: Vec<_> = validator
                    .iter_errors(value)
                    .map(|e| format!("  - {e}"))
                    .collect();
                assert!(
                    errors.is_empty(),
                    "Schema validation failed for '{}':\n{}",
                    $def_name,
                    errors.join("\n"),
                );
            }
        }};
    }

    macro_rules! assert_schema_valid {
        ($def_name:expr, $instance:expr) => {{
            let validator = def_validator($def_name);
            let value = serde_json::to_value(&$instance).unwrap();
            let errors: Vec<_> = validator
                .iter_errors(&value)
                .map(|e| format!("  - {e}"))
                .collect();
            assert!(
                errors.is_empty(),
                "Schema validation failed for '{}':\n{}",
                $def_name,
                errors.join("\n"),
            );
        }};
    }

    #[test]
    fn schema_sync_common_response() {
        assert_schema_valid!(
            "CommonResponse",
            CommonResponse {
                ok: true,
                message: "m".into(),
                severity: Severity::Info,
            }
        );
    }

    #[test]
    fn schema_sync_init_response() {
        assert_schema_valid!(
            "InitResponse",
            InitResponse {
                ok: true,
                message: "m".into(),
                severity: Severity::Info,
                config_complete: true,
                missing_fields: vec![],
            }
        );
    }

    #[test]
    fn schema_sync_config() {
        assert_schema_valid!(
            "Config",
            Config {
                field_name: "f".into(),
                category: Some("c".into()),
                description: "d".into(),
                config_type: ConfigType::Scalar(ScalarType::String),
                required: true,
                default: serde_json::Value::Null,
                value: serde_json::Value::Null,
            }
        );
    }

    #[test]
    fn schema_sync_tool_config_response() {
        assert_schema_valid!(
            "ToolConfigResponse",
            ToolConfigResponse {
                ok: true,
                message: "m".into(),
                severity: Severity::Info,
                configs: vec![],
            }
        );
    }

    #[test]
    fn schema_sync_create_workfile_response() {
        assert_schema_valid!(
            "CreateWorkfileResponse",
            CreateWorkfileResponse {
                ok: true,
                message: "m".into(),
                severity: Severity::Info,
                path: "/p".into(),
            }
        );
    }

    #[test]
    fn schema_sync_device() {
        assert_schema_valid!(
            "Device",
            Device {
                tag: "t".into(),
                key: "k".into(),
            }
        );
    }

    #[test]
    fn schema_sync_list_devices_response() {
        assert_schema_valid!(
            "ListDevicesResponse",
            ListDevicesResponse {
                ok: true,
                message: "m".into(),
                severity: Severity::Info,
                devices: vec![],
            }
        );
    }

    #[test]
    fn schema_sync_add_response() {
        assert_schema_valid!(
            "AddResponse",
            AddResponse {
                ok: true,
                message: "m".into(),
                severity: Severity::Info,
                key: "k".into(),
                path: vec!["r".into()],
            }
        );
    }

    #[test]
    fn schema_sync_property() {
        assert_schema_valid!(
            "Property",
            Property {
                kind: "property".into(),
                key: "k".into(),
                prop_type: Types::String,
                value: serde_json::json!("v"),
            }
        );
    }

    #[test]
    fn schema_sync_node() {
        assert_schema_valid!(
            "Node",
            Node {
                kind: "node".into(),
                key: "k".into(),
                properties: vec![],
                children: vec![],
                alias: Some(vec!["a".into()]),
            }
        );
    }

    #[test]
    fn schema_sync_read_response() {
        assert_union_schema_valid!(
            "ReadResponse",
            ReadResponse::Node(Node {
                kind: "node".into(),
                key: "k".into(),
                properties: vec![],
                children: vec![],
                alias: Some(vec!["a".into()]),
            }),
            ReadResponse::Property(Property {
                kind: "property".into(),
                key: "k".into(),
                prop_type: Types::String,
                value: serde_json::json!("v"),
            }),
        );
    }

    #[test]
    fn schema_sync_delete_preview() {
        assert_schema_valid!(
            "DeletePreview",
            DeletePreview {
                ok: true,
                message: "m".into(),
                severity: Severity::Info,
                node_count: 0,
                property_count: 0,
                paths: vec![],
            }
        );
    }

    #[test]
    fn schema_sync_delete_response() {
        assert_union_schema_valid!(
            "DeleteResponse",
            DeleteResponse::Preview(DeletePreview {
                ok: true,
                message: "m".into(),
                severity: Severity::Info,
                node_count: 0,
                property_count: 0,
                paths: vec![],
            }),
            DeleteResponse::Common(CommonResponse {
                ok: true,
                message: "m".into(),
                severity: Severity::Info,
            }),
        );
    }

    #[test]
    fn schema_sync_error() {
        assert_schema_valid!(
            "Error",
            ValidationError {
                kind: "generic".into(),
                path: vec![],
                message: "m".into(),
            }
        );
    }

    #[test]
    fn schema_sync_validation_response() {
        assert_schema_valid!(
            "ValidationResponse",
            ValidationResponse {
                errors: vec![],
                warnings: vec![],
            }
        );
    }

    #[test]
    fn schema_sync_intelligence_arg() {
        assert_schema_valid!(
            "IntelligenceArg",
            IntelligenceArg {
                name: "n".into(),
                description: "d".into(),
                required: true,
                kind: Some("k".into()),
            }
        );
    }

    #[test]
    fn schema_sync_intelligence() {
        assert_schema_valid!(
            "Intelligence",
            Intelligence {
                kind: "k".into(),
                args: vec![],
            }
        );
    }

    #[test]
    fn schema_sync_list_intelligence_response() {
        assert_schema_valid!(
            "ListIntelligenceResponse",
            ListIntelligenceResponse {
                ok: true,
                message: "m".into(),
                severity: Severity::Info,
                intelligence: vec![],
            }
        );
    }

    #[test]
    fn schema_sync_suggestion() {
        assert_schema_valid!(
            "Suggestion",
            Suggestion {
                value: "v".into(),
                display_string: Some("d".into()),
            }
        );
    }

    #[test]
    fn schema_sync_suggest_response() {
        assert_schema_valid!(
            "SuggestResponse",
            SuggestResponse {
                ok: true,
                message: "m".into(),
                severity: Severity::Info,
                suggestions: vec![],
            }
        );
    }
}
