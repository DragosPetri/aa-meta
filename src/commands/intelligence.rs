use crate::error::AttachMetaError;
use crate::meta_intelligence;
use crate::protocol::manifest::{CommandName, Manifest};
use crate::protocol::responses::{ListIntelligenceResponse, Severity, SuggestResponse};
use crate::transport;

pub fn run(
    cmd: CommandName,
    positionals: &[String],
    flags: &serde_json::Value,
    tool_ctx: Option<&Manifest>,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    match cmd {
        CommandName::ListIntelligence => run_list_intelligence(tool_ctx),
        CommandName::Suggest => run_suggest(positionals, flags, tool_ctx),
        _ => unreachable!(),
    }
}

fn run_list_intelligence(
    tool_ctx: Option<&Manifest>,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    let meta = meta_intelligence::meta_intelligences();

    let tool_intelligence = if let Some(li_mapping) =
        tool_ctx.and_then(|m| m.get_command(CommandName::ListIntelligence))
    {
        let li_raw = transport::invoke(li_mapping, &[])?;
        let li_response: ListIntelligenceResponse =
            serde_json::from_value(li_raw.clone()).map_err(|e| {
                AttachMetaError::InternalError(format!(
                    "failed to parse list-intelligence response: {e}"
                ))
            })?;
        if !li_response.ok {
            return Err(AttachMetaError::ProtocolError);
        }
        li_response.intelligence
    } else {
        vec![]
    };

    let merged = meta_intelligence::merge_intelligence(meta, tool_intelligence);

    let response = ListIntelligenceResponse {
        ok: true,
        message: "intelligence".to_string(),
        severity: Severity::Info,
        intelligence: merged,
    };

    serde_json::to_value(&response)
        .map_err(|e| AttachMetaError::InternalError(format!("failed to serialize response: {e}")))
}

fn run_suggest(
    positionals: &[String],
    flags: &serde_json::Value,
    tool_ctx: Option<&Manifest>,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    let kind = positionals.first().ok_or_else(|| {
        AttachMetaError::InputError("suggest requires a kind argument".to_string())
    })?;

    if meta_intelligence::is_meta_kind(kind) {
        let suggestions = meta_intelligence::meta_suggest(kind, &positionals[1..])?;
        let response = SuggestResponse {
            ok: true,
            message: "suggestions".to_string(),
            severity: Severity::Info,
            suggestions,
        };
        return serde_json::to_value(&response).map_err(|e| {
            AttachMetaError::InternalError(format!("failed to serialize response: {e}"))
        });
    }

    let manifest = tool_ctx.ok_or_else(|| {
        AttachMetaError::InputError(format!(
            "suggest kind '{kind}' is not available — no tool configured"
        ))
    })?;

    let suggest_mapping = manifest.get_command(CommandName::Suggest).ok_or_else(|| {
        AttachMetaError::ManifestError("command 'suggest' not in manifest".to_string())
    })?;

    let mut extra_args: Vec<String> = positionals.to_vec();
    if let Some(obj) = flags.as_object() {
        for (key, val) in obj {
            if val.is_boolean() {
                if val.as_bool() == Some(true) {
                    extra_args.push(format!("--{key}"));
                }
            } else if let Some(s) = val.as_str() {
                extra_args.push(format!("--{key}"));
                extra_args.push(s.to_string());
            } else if let Some(arr) = val.as_array() {
                extra_args.push(format!("--{key}"));
                extra_args.extend(arr.iter().filter_map(|v| v.as_str()).map(String::from));
            }
        }
    }

    transport::invoke(suggest_mapping, &extra_args)
}
