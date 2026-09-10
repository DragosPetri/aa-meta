use serde_json::{Value, json};

use crate::error::AttachMetaError;
use crate::protocol::base_schema;
use crate::protocol::manifest::{CommandMapping, CommandName};

#[derive(Debug)]
pub struct ParsedInput {
    pub flags_json: Value,
    pub positionals: Vec<String>,
    pub forwarded_argv: Vec<String>,
}

pub fn collect_flag_names(cmd: CommandName, tool_args: Option<&Value>) -> Vec<String> {
    let base = base_schema::base_schema(cmd);
    let mut flags: Vec<String> = Vec::new();
    if let Some(props) = base.get("properties").and_then(|p| p.as_object()) {
        for (name, prop) in props {
            if prop.get("x-positional").and_then(|v| v.as_bool()) != Some(true) {
                flags.push(name.clone());
            }
        }
    }
    if let Some(ta) = tool_args {
        if let Some(props) = ta.get("properties").and_then(|p| p.as_object()) {
            for key in props.keys() {
                if !flags.contains(key) {
                    flags.push(key.clone());
                }
            }
        }
    }
    flags
}

pub fn parse_command_args(
    cmd: CommandName,
    mapping: &CommandMapping,
    raw_args: &[String],
) -> std::result::Result<ParsedInput, AttachMetaError> {
    let base = base_schema::base_schema(cmd);
    let tool_args = mapping.args.as_ref();

    let mut flags: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut positionals: Vec<String> = Vec::new();
    let mut forwarded_argv: Vec<String> = mapping.argv.clone();

    let known_flags = collect_flag_names(cmd, tool_args);
    // Check for collisions between base and tool args
    if let Some(ta) = tool_args {
        if let Some(base_props) = base.get("properties").and_then(|p| p.as_object()) {
            if let Some(tool_props) = ta.get("properties").and_then(|p| p.as_object()) {
                for key in tool_props.keys() {
                    if base_props.contains_key(key) {
                        return Err(AttachMetaError::ManifestError(format!(
                            "tool arg '--{key}' collides with a protocol flag"
                        )));
                    }
                }
            }
        }
    }

    let base_opt = Some(base);

    let mut i = 0;
    while i < raw_args.len() {
        let arg = &raw_args[i];
        if let Some(flag_name) = arg.strip_prefix("--") {
            if flag_name.is_empty() {
                i += 1;
                positionals.extend(raw_args[i..].iter().cloned());
                break;
            }

            let is_bool = is_boolean_flag(flag_name, &base_opt, tool_args);
            let is_array = is_array_flag(flag_name, &base_opt, tool_args);

            if is_bool {
                flags.insert(flag_name.to_string(), json!(true));
                forwarded_argv.push(arg.clone());
            } else if is_array {
                // Consume all following non-flag tokens as array elements
                let mut values: Vec<Value> = Vec::new();
                forwarded_argv.push(arg.clone());
                i += 1;
                while i < raw_args.len() && !raw_args[i].starts_with("--") {
                    values.push(json!(raw_args[i].as_str()));
                    forwarded_argv.push(raw_args[i].clone());
                    i += 1;
                }
                if values.is_empty() {
                    return Err(AttachMetaError::InputError(format!(
                        "flag '--{flag_name}' requires at least one value"
                    )));
                }
                flags.insert(flag_name.to_string(), Value::Array(values));
                continue; // i already advanced past the values
            } else if i + 1 < raw_args.len() {
                i += 1;
                flags.insert(flag_name.to_string(), json!(&raw_args[i]));
                forwarded_argv.push(arg.clone());
                forwarded_argv.push(raw_args[i].clone());
            } else {
                return Err(AttachMetaError::InputError(format!(
                    "flag '--{flag_name}' requires a value"
                )));
            }
        } else {
            positionals.push(arg.clone());
            forwarded_argv.push(arg.clone());
        }
        i += 1;
    }

    Ok(ParsedInput {
        flags_json: Value::Object(flags),
        positionals,
        forwarded_argv,
    })
}

pub fn flag_type_in_schema(
    name: &str,
    cmd: CommandName,
    tool_args: Option<&Value>,
) -> Option<String> {
    let base = base_schema::base_schema(cmd);
    flag_type(name, &Some(base), tool_args).map(|s| s.to_string())
}

fn is_boolean_flag(name: &str, base: &Option<Value>, tool_args: Option<&Value>) -> bool {
    flag_type(name, base, tool_args) == Some("boolean")
}

fn is_array_flag(name: &str, base: &Option<Value>, tool_args: Option<&Value>) -> bool {
    flag_type(name, base, tool_args) == Some("array")
}

fn flag_type<'a>(
    name: &str,
    base: &'a Option<Value>,
    tool_args: Option<&'a Value>,
) -> Option<&'a str> {
    // x-positional in the base schema marks this as a positional arg, not a flag
    if let Some(b) = base.as_ref() {
        if b.get("properties")
            .and_then(|p| p.get(name))
            .and_then(|prop| prop.get("x-positional"))
            .and_then(|v| v.as_bool())
            == Some(true)
        {
            return None;
        }
    }
    for schema in [base.as_ref(), tool_args] {
        if let Some(s) = schema {
            if let Some(t) = s
                .get("properties")
                .and_then(|p| p.get(name))
                .and_then(|prop| prop.get("type"))
                .and_then(|t| t.as_str())
            {
                return Some(t);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::manifest::CommandMapping;

    fn mapping(args: Option<Value>) -> CommandMapping {
        CommandMapping {
            description: None,
            argv: vec!["tool".to_string(), "add".to_string()],
            args,
            timeout_ms: None,
            completions: None,
        }
    }

    #[test]
    fn parse_add_key_as_positional() {
        let m = mapping(None);
        let raw = vec!["my_device".to_string()];
        let parsed = parse_command_args(CommandName::Add, &m, &raw).unwrap();
        assert_eq!(parsed.positionals, vec!["my_device"]);
        assert!(!parsed.flags_json.as_object().unwrap().contains_key("key"));
    }

    #[test]
    fn parse_with_positionals() {
        let m = mapping(None);
        let raw = vec!["soc".to_string(), "spi".to_string(), "--force".to_string()];
        let parsed = parse_command_args(CommandName::Delete, &m, &raw).unwrap();
        assert_eq!(parsed.positionals, vec!["soc", "spi"]);
        assert_eq!(parsed.flags_json["force"], true);
    }

    #[test]
    fn parse_bool_flag() {
        let m = mapping(None);
        let raw = vec!["--force".to_string()];
        let parsed = parse_command_args(CommandName::Delete, &m, &raw).unwrap();
        assert_eq!(parsed.flags_json["force"], true);
    }

    #[test]
    fn tool_arg_collision_rejected() {
        let tool_args = json!({
            "properties": {
                "key": { "type": "string" }
            }
        });
        let m = mapping(Some(tool_args));
        let raw = vec![];
        let err = parse_command_args(CommandName::Add, &m, &raw).unwrap_err();
        assert!(matches!(err, AttachMetaError::ManifestError(_)));
    }

    #[test]
    fn tool_declared_extra_flag() {
        let tool_args = json!({
            "properties": {
                "bus_id": { "type": "string" }
            }
        });
        let m = mapping(Some(tool_args));
        let raw = vec!["k".to_string(), "--bus_id".to_string(), "b1".to_string()];
        let parsed = parse_command_args(CommandName::Add, &m, &raw).unwrap();
        assert_eq!(parsed.positionals, vec!["k"]);
        assert_eq!(parsed.flags_json["bus_id"], "b1");
    }
}
