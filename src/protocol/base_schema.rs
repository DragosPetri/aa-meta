use serde_json::{Value, json};

use super::manifest::CommandName;

macro_rules! base_schema_file {
    ($path:expr) => {
        serde_json::from_str(include_str!($path)).expect(concat!("invalid JSON in ", $path))
    };
}

pub fn base_schema(cmd: CommandName) -> Value {
    match cmd {
        CommandName::Add => {
            base_schema_file!("../../docs/schemas/command_base_schemas/add.schema.json")
        }
        CommandName::Read => {
            base_schema_file!("../../docs/schemas/command_base_schemas/read.schema.json")
        }
        CommandName::Update => {
            base_schema_file!("../../docs/schemas/command_base_schemas/update.schema.json")
        }
        CommandName::Delete => {
            base_schema_file!("../../docs/schemas/command_base_schemas/delete.schema.json")
        }
        CommandName::Move => {
            base_schema_file!("../../docs/schemas/command_base_schemas/move.schema.json")
        }
        CommandName::Rename => {
            base_schema_file!("../../docs/schemas/command_base_schemas/rename.schema.json")
        }
        CommandName::Alias => {
            base_schema_file!("../../docs/schemas/command_base_schemas/alias.schema.json")
        }
        CommandName::Validate => {
            base_schema_file!("../../docs/schemas/command_base_schemas/validate.schema.json")
        }
        CommandName::Suggest => {
            base_schema_file!("../../docs/schemas/command_base_schemas/suggest.schema.json")
        }
        CommandName::ToolConfigGet => {
            base_schema_file!("../../docs/schemas/command_base_schemas/tool-config-get.schema.json")
        }
        CommandName::ToolConfigSet => {
            base_schema_file!("../../docs/schemas/command_base_schemas/tool-config-set.schema.json")
        }
        CommandName::CreateWorkfile => {
            base_schema_file!("../../docs/schemas/command_base_schemas/create-workfile.schema.json")
        }
        CommandName::ListDevices => {
            base_schema_file!("../../docs/schemas/command_base_schemas/list-devices.schema.json")
        }
        CommandName::Generate => {
            base_schema_file!("../../docs/schemas/command_base_schemas/generate.schema.json")
        }
        CommandName::Build => {
            base_schema_file!("../../docs/schemas/command_base_schemas/build.schema.json")
        }
        CommandName::Deploy => {
            base_schema_file!("../../docs/schemas/command_base_schemas/deploy.schema.json")
        }
        CommandName::ListIntelligence => base_schema_file!(
            "../../docs/schemas/command_base_schemas/list-intelligence.schema.json"
        ),
    }
}

pub fn effective_schema(cmd: CommandName, tool_args: Option<&Value>) -> Value {
    let base = base_schema(cmd);
    match tool_args {
        Some(ta) => json!({ "allOf": [base, ta] }),
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_base_requires_key_or_name() {
        let s = base_schema(CommandName::Add);
        let any_of = s["anyOf"].as_array().unwrap();
        assert_eq!(any_of.len(), 2);
    }

    #[test]
    fn each_base_schema_has_at_most_one_x_positional() {
        for cmd in CommandName::ALL {
            let schema = base_schema(*cmd);
            let count = schema
                .get("properties")
                .and_then(|p| p.as_object())
                .map(|props| {
                    props
                        .values()
                        .filter(|prop| {
                            prop.get("x-positional").and_then(|v| v.as_bool()) == Some(true)
                        })
                        .count()
                })
                .unwrap_or(0);
            assert!(
                count <= 1,
                "base schema for '{}' has {count} x-positional properties; at most 1 is allowed",
                cmd
            );
        }
    }

    #[test]
    fn effective_merges_allof() {
        let tool_args = json!({
            "properties": {
                "bus_id": { "type": "string" }
            }
        });
        let eff = effective_schema(CommandName::Add, Some(&tool_args));
        let all_of = eff["allOf"].as_array().unwrap();
        assert_eq!(all_of.len(), 2);
    }

    #[test]
    fn all_base_schemas_parse_as_valid_json_objects() {
        for cmd in CommandName::ALL {
            let schema = base_schema(*cmd);
            assert!(
                schema.is_object(),
                "base schema for '{}' is not a JSON object",
                cmd
            );
        }
    }

    #[test]
    fn every_command_has_a_base_schema_file() {
        for cmd in CommandName::ALL {
            let schema = base_schema(*cmd);
            assert!(
                schema.get("type").is_some(),
                "base schema for '{}' is missing 'type' field",
                cmd
            );
        }
    }
}
