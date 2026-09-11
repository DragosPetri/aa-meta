use crate::error::AttachMetaError;
use crate::meta_intelligence;
use crate::protocol::manifest::{CommandName, Manifest};
use crate::protocol::responses::{ListIntelligenceResponse, Severity, SuggestResponse};
use crate::transport;

pub fn run(
    cmd: CommandName,
    positionals: &[String],
    _flags: &serde_json::Value,
    tool_ctx: Option<&Manifest>,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    match cmd {
        CommandName::ListIntelligence => run_list_intelligence(tool_ctx),
        CommandName::Suggest => run_suggest(positionals, tool_ctx),
        _ => unreachable!(),
    }
}

fn run_list_intelligence(
    tool_ctx: Option<&Manifest>,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    let meta = meta_intelligence::meta_intelligences();

    let tool_intelligence =
        if let Some(li_mapping) = tool_ctx.and_then(|m| m.get_command(CommandName::ListIntelligence))
        {
            let li_raw = transport::invoke(li_mapping, &[])?;
            let li_response: ListIntelligenceResponse =
                serde_json::from_value(li_raw).map_err(|e| {
                    AttachMetaError::InternalError(format!(
                        "failed to parse list-intelligence response: {e}"
                    ))
                })?;
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

    let li_mapping = manifest
        .get_command(CommandName::ListIntelligence)
        .ok_or_else(|| {
            AttachMetaError::ManifestError(
                "command 'list-intelligence' not in manifest".to_string(),
            )
        })?;

    let li_raw = transport::invoke(li_mapping, &[])?;
    let li_response: ListIntelligenceResponse = serde_json::from_value(li_raw).map_err(|e| {
        AttachMetaError::InternalError(format!("failed to parse list-intelligence response: {e}"))
    })?;

    let advertised = li_response.intelligence.iter().any(|i| i.kind == *kind);
    if !advertised {
        return Err(AttachMetaError::InputError(format!(
            "suggest kind '{kind}' is not advertised by list-intelligence"
        )));
    }

    let suggest_mapping = manifest.get_command(CommandName::Suggest).ok_or_else(|| {
        AttachMetaError::ManifestError("command 'suggest' not in manifest".to_string())
    })?;

    transport::invoke(suggest_mapping, positionals)
}
