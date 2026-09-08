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
        flags.extend(props.keys().cloned());
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

    let mut i = 0;
    while i < raw_args.len() {
        let arg = &raw_args[i];
        if let Some(flag_name) = arg.strip_prefix("--") {
            if flag_name.is_empty() {
                i += 1;
                positionals.extend(raw_args[i..].iter().cloned());
                break;
            }

            let is_bool_flag = is_boolean_flag(flag_name, &Some(base.clone()), tool_args);

            if is_bool_flag {
                flags.insert(flag_name.to_string(), json!(true));
                forwarded_argv.push(arg.clone());
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

pub fn is_bool_flag_in_schema(name: &str, cmd: CommandName, tool_args: Option<&Value>) -> bool {
    let base = base_schema::base_schema(cmd);
    is_boolean_flag(name, &Some(base), tool_args)
}

fn is_boolean_flag(name: &str, base: &Option<Value>, tool_args: Option<&Value>) -> bool {
    for schema in [base.as_ref(), tool_args] {
        if let Some(s) = schema {
            if let Some(prop) = s.get("properties").and_then(|p| p.get(name)) {
                if prop.get("type").and_then(|t| t.as_str()) == Some("boolean") {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::manifest::CommandMapping;

    fn mapping(args: Option<Value>) -> CommandMapping {
        CommandMapping {
            argv: vec!["tool".to_string(), "add".to_string()],
            args,
            timeout_ms: None,
            completions: None,
        }
    }

    #[test]
    fn parse_add_with_key() {
        let m = mapping(None);
        let raw = vec!["--key".to_string(), "my_device".to_string()];
        let parsed = parse_command_args(CommandName::Add, &m, &raw).unwrap();
        assert_eq!(parsed.flags_json["key"], "my_device");
        assert!(parsed.positionals.is_empty());
    }

    #[test]
    fn parse_with_positionals() {
        let m = mapping(None);
        let raw = vec![
            "soc".to_string(),
            "spi".to_string(),
            "--key".to_string(),
            "k".to_string(),
        ];
        let parsed = parse_command_args(CommandName::Add, &m, &raw).unwrap();
        assert_eq!(parsed.positionals, vec!["soc", "spi"]);
        assert_eq!(parsed.flags_json["key"], "k");
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
        let raw = vec![
            "--key".to_string(),
            "k".to_string(),
            "--bus_id".to_string(),
            "b1".to_string(),
        ];
        let parsed = parse_command_args(CommandName::Add, &m, &raw).unwrap();
        assert_eq!(parsed.flags_json["key"], "k");
        assert_eq!(parsed.flags_json["bus_id"], "b1");
    }
}
