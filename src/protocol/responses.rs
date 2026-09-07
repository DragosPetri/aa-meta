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

static RESPONSES_SCHEMA: &str = include_str!("../../docs/schemas/responses.schema.json");

#[cfg(test)]
mod tests {
    use super::*;

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
    // instance of the struct, serializes it, and checks that every field
    // declared in the schema's $def is present (and vice-versa).

    fn schema_defs() -> serde_json::Map<String, serde_json::Value> {
        let root: serde_json::Value = serde_json::from_str(RESPONSES_SCHEMA).unwrap();
        root.get("$defs")
            .unwrap()
            .as_object()
            .unwrap()
            .clone()
    }

    /// Collect all property names from a $def, resolving one level of
    /// `allOf` → `$ref` → `#/$defs/<Name>` into the parent's properties.
    fn schema_fields(defs: &serde_json::Map<String, serde_json::Value>, def_name: &str) -> std::collections::BTreeSet<String> {
        let def = &defs[def_name];
        let mut fields = std::collections::BTreeSet::new();

        // Directly declared properties
        if let Some(props) = def.get("properties").and_then(|p| p.as_object()) {
            fields.extend(props.keys().cloned());
        }

        // allOf → $ref to other $defs (one level)
        if let Some(all_of) = def.get("allOf").and_then(|a| a.as_array()) {
            for entry in all_of {
                if let Some(r) = entry.get("$ref").and_then(|r| r.as_str()) {
                    if let Some(ref_name) = r.strip_prefix("#/$defs/") {
                        fields.extend(schema_fields(defs, ref_name));
                    }
                }
            }
        }

        fields
    }

    /// Collect the field names a Rust struct actually serializes.
    fn serialized_fields(value: &serde_json::Value) -> std::collections::BTreeSet<String> {
        value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }

    macro_rules! assert_fields_match {
        ($def_name:expr, $instance:expr) => {{
            let defs = schema_defs();
            let expected = schema_fields(&defs, $def_name);
            let actual = serialized_fields(&serde_json::to_value(&$instance).unwrap());

            let missing_from_rust: Vec<_> = expected.difference(&actual).collect();
            let extra_in_rust: Vec<_> = actual.difference(&expected).collect();

            assert!(
                missing_from_rust.is_empty() && extra_in_rust.is_empty(),
                "Schema sync mismatch for '{}':\n  \
                 in schema but not in struct: {:?}\n  \
                 in struct but not in schema: {:?}",
                $def_name,
                missing_from_rust,
                extra_in_rust,
            );
        }};
    }

    #[test]
    fn schema_sync_common_response() {
        assert_fields_match!("CommonResponse", CommonResponse {
            ok: true,
            message: "m".into(),
            severity: Severity::Info,
        });
    }

    #[test]
    fn schema_sync_init_response() {
        assert_fields_match!("InitResponse", InitResponse {
            ok: true,
            message: "m".into(),
            severity: Severity::Info,
            config_complete: true,
            missing_fields: vec![],
        });
    }

    #[test]
    fn schema_sync_config() {
        assert_fields_match!("Config", Config {
            field_name: "f".into(),
            category: Some("c".into()),
            description: "d".into(),
            config_type: ConfigType::Scalar(ScalarType::String),
            required: true,
            default: serde_json::Value::Null,
        });
    }

    #[test]
    fn schema_sync_tool_config_response() {
        assert_fields_match!("ToolConfigResponse", ToolConfigResponse {
            ok: true,
            message: "m".into(),
            severity: Severity::Info,
            configs: vec![],
        });
    }

    #[test]
    fn schema_sync_create_workfile_response() {
        assert_fields_match!("CreateWorkfileResponse", CreateWorkfileResponse {
            ok: true,
            message: "m".into(),
            severity: Severity::Info,
            path: "/p".into(),
        });
    }

    #[test]
    fn schema_sync_device() {
        assert_fields_match!("Device", Device {
            tag: "t".into(),
            key: "k".into(),
        });
    }

    #[test]
    fn schema_sync_list_devices_response() {
        assert_fields_match!("ListDevicesResponse", ListDevicesResponse {
            ok: true,
            message: "m".into(),
            severity: Severity::Info,
            devices: vec![],
        });
    }

    #[test]
    fn schema_sync_add_response() {
        assert_fields_match!("AddResponse", AddResponse {
            ok: true,
            message: "m".into(),
            severity: Severity::Info,
            key: "k".into(),
            path: vec!["r".into()],
        });
    }

    #[test]
    fn schema_sync_property() {
        assert_fields_match!("Property", Property {
            kind: "property".into(),
            key: "k".into(),
            prop_type: Types::String,
            value: serde_json::json!("v"),
        });
    }

    #[test]
    fn schema_sync_node() {
        assert_fields_match!("Node", Node {
            kind: "node".into(),
            key: "k".into(),
            properties: vec![],
            children: vec![],
            alias: Some(vec!["a".into()]),
        });
    }

    #[test]
    fn schema_sync_delete_preview() {
        assert_fields_match!("DeletePreview", DeletePreview {
            ok: true,
            message: "m".into(),
            severity: Severity::Info,
            node_count: 0,
            property_count: 0,
            paths: vec![],
        });
    }

    #[test]
    fn schema_sync_error() {
        assert_fields_match!("Error", ValidationError {
            kind: "generic".into(),
            path: vec![],
            message: "m".into(),
        });
    }

    #[test]
    fn schema_sync_validation_response() {
        assert_fields_match!("ValidationResponse", ValidationResponse {
            errors: vec![],
            warnings: vec![],
        });
    }

    #[test]
    fn schema_sync_intelligence_arg() {
        assert_fields_match!("IntelligenceArg", IntelligenceArg {
            name: "n".into(),
            description: "d".into(),
            required: true,
            kind: Some("k".into()),
        });
    }

    #[test]
    fn schema_sync_intelligence() {
        assert_fields_match!("Intelligence", Intelligence {
            kind: "k".into(),
            args: vec![],
        });
    }

    #[test]
    fn schema_sync_list_intelligence_response() {
        assert_fields_match!("ListIntelligenceResponse", ListIntelligenceResponse {
            ok: true,
            message: "m".into(),
            severity: Severity::Info,
            intelligence: vec![],
        });
    }

    #[test]
    fn schema_sync_suggestion() {
        assert_fields_match!("Suggestion", Suggestion {
            value: "v".into(),
            display_string: Some("d".into()),
        });
    }

    #[test]
    fn schema_sync_suggest_response() {
        assert_fields_match!("SuggestResponse", SuggestResponse {
            ok: true,
            message: "m".into(),
            severity: Severity::Info,
            suggestions: vec![],
        });
    }
}
