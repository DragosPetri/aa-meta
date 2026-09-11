use anyhow::Result;
use jsonschema::{Validator, options};
use serde_json::Value;

use crate::error::AttachMetaError;

static MANIFEST_SCHEMA: &str = include_str!("../docs/schemas/manifest.schema.json");

pub fn manifest_validator() -> Result<Validator> {
    let schema: Value =
        serde_json::from_str(MANIFEST_SCHEMA).expect("embedded manifest schema is invalid JSON");
    let mut opts = options();
    opts.with_draft(jsonschema::Draft::Draft202012);
    opts.build(&schema)
        .map_err(|e| anyhow::anyhow!("failed to compile manifest meta-schema: {e}"))
}

pub fn validate_manifest(manifest_json: &Value) -> std::result::Result<(), AttachMetaError> {
    let validator =
        manifest_validator().map_err(|e| AttachMetaError::InternalError(e.to_string()))?;

    let result = validator.apply(manifest_json);
    if !result.flag() {
        let errors: Vec<String> = validator
            .iter_errors(manifest_json)
            .map(|e| format!("{} at {}", e, e.instance_path))
            .collect();
        return Err(AttachMetaError::ManifestError(format!(
            "manifest meta-schema validation failed:\n  {}",
            errors.join("\n  ")
        )));
    }

    check_no_required_in_args(manifest_json)?;
    check_no_completion_flag_shadows_command(manifest_json)?;

    Ok(())
}

fn check_no_required_in_args(manifest: &Value) -> std::result::Result<(), AttachMetaError> {
    let commands = match manifest.get("commands").and_then(|c| c.as_object()) {
        Some(c) => c,
        None => return Ok(()),
    };

    for (cmd_name, mapping) in commands {
        if let Some(args) = mapping.get("args") {
            if args.get("required").is_some() {
                return Err(AttachMetaError::ManifestError(format!(
                    "command '{cmd_name}' declares a top-level 'required' in args — \
                     tool-declared args may only add optional properties"
                )));
            }
        }
    }

    Ok(())
}

fn check_no_completion_flag_shadows_command(
    manifest: &Value,
) -> std::result::Result<(), AttachMetaError> {
    let commands = match manifest.get("commands").and_then(|c| c.as_object()) {
        Some(c) => c,
        None => return Ok(()),
    };

    for (cmd_name, mapping) in commands {
        let collides = mapping
            .get("args")
            .and_then(|a| a.get("properties"))
            .and_then(|p| p.as_object())
            .map(|props| props.contains_key(cmd_name.as_str()))
            .unwrap_or(false);

        if collides {
            return Err(AttachMetaError::ManifestError(format!(
                "command '{cmd_name}' declares a flag named '{cmd_name}' — \
                 flag names must not equal their command name (reserved for positional completion)"
            )));
        }
    }

    Ok(())
}

pub fn validate_input(schema: &Value, input: &Value) -> std::result::Result<(), AttachMetaError> {
    let mut opts = options();
    opts.with_draft(jsonschema::Draft::Draft202012);
    let validator = opts
        .build(schema)
        .map_err(|e| AttachMetaError::InputError(format!("failed to compile input schema: {e}")))?;

    let result = validator.apply(input);
    if !result.flag() {
        let errors: Vec<String> = validator
            .iter_errors(input)
            .map(|e| format!("{}", e))
            .collect();
        return Err(AttachMetaError::InputError(format!(
            "input validation failed:\n  {}",
            errors.join("\n  ")
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn minimal_manifest() -> Value {
        json!({
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
        })
    }

    #[test]
    fn valid_manifest_passes() {
        validate_manifest(&minimal_manifest()).unwrap();
    }

    #[test]
    fn missing_required_command_fails() {
        let mut m = minimal_manifest();
        m["commands"].as_object_mut().unwrap().remove("add");
        let err = validate_manifest(&m).unwrap_err();
        assert!(matches!(err, AttachMetaError::ManifestError(_)));
    }

    #[test]
    fn top_level_required_in_args_rejected() {
        let mut m = minimal_manifest();
        m["commands"]["add"]["args"] = json!({
            "properties": {
                "bus_id": { "type": "string" }
            },
            "required": ["bus_id"]
        });
        let err = validate_manifest(&m).unwrap_err();
        match err {
            AttachMetaError::ManifestError(msg) => {
                assert!(msg.contains("top-level 'required'"), "got: {msg}");
            }
            other => panic!("expected ManifestError, got: {other:?}"),
        }
    }

    #[test]
    fn unknown_command_name_rejected() {
        let mut m = minimal_manifest();
        m["commands"]["bogus-command"] = json!({ "argv": ["tool", "bogus"] });
        let err = validate_manifest(&m).unwrap_err();
        assert!(matches!(err, AttachMetaError::ManifestError(_)));
    }

    #[test]
    fn input_validation_passes() {
        let schema = json!({
            "type": "object",
            "properties": {
                "key": { "type": "string" }
            },
            "required": ["key"]
        });
        let input = json!({ "key": "foo" });
        validate_input(&schema, &input).unwrap();
    }

    #[test]
    fn input_validation_fails_missing_required() {
        let schema = json!({
            "type": "object",
            "properties": {
                "key": { "type": "string" }
            },
            "required": ["key"]
        });
        let input = json!({});
        let err = validate_input(&schema, &input).unwrap_err();
        assert!(matches!(err, AttachMetaError::InputError(_)));
    }

    #[test]
    fn completion_flag_same_as_command_name_rejected() {
        let mut m = minimal_manifest();
        // "add" flag declared in the "add" command — reserved for positional completion
        m["commands"]["add"] = json!({
            "argv": ["tool", "add"],
            "args": {
                "properties": {
                    "add": { "type": "string" }
                }
            }
        });
        let err = validate_manifest(&m).unwrap_err();
        match err {
            AttachMetaError::ManifestError(msg) => {
                assert!(
                    msg.contains("flag names must not equal their command name"),
                    "got: {msg}"
                );
            }
            other => panic!("expected ManifestError, got: {other:?}"),
        }
    }

    #[test]
    fn manifest_with_args_and_completions_passes() {
        let mut m = minimal_manifest();
        m["commands"]["add"] = json!({
            "argv": ["tool", "add"],
            "args": {
                "properties": {
                    "bus_id": { "type": "string" }
                }
            },
            "timeout_ms": 5000,
            "completions": [
                { "arg": "add", "kind": "device-key" }
            ]
        });
        validate_manifest(&m).unwrap();
    }
}
