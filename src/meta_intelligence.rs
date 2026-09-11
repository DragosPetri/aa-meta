use crate::error::AttachMetaError;
use crate::protocol::responses::{Intelligence, IntelligenceArg, Suggestion};

pub const RESERVED_KINDS: &[&str] = &["attachable"];

pub fn meta_intelligences() -> Vec<Intelligence> {
    vec![Intelligence {
        kind: "attachable".to_string(),
        args: vec![IntelligenceArg {
            name: "partial".to_string(),
            description: "partial binary name to filter".to_string(),
            required: false,
            kind: None,
        }],
    }]
}

pub fn is_meta_kind(kind: &str) -> bool {
    RESERVED_KINDS.contains(&kind)
}

pub fn meta_suggest(kind: &str, args: &[String]) -> Result<Vec<Suggestion>, AttachMetaError> {
    match kind {
        "attachable" => Ok(scan_path_for_attachables(args.first().map(|s| s.as_str()))),
        _ => Err(AttachMetaError::InputError(format!(
            "unknown meta intelligence kind '{kind}'"
        ))),
    }
}

pub fn merge_intelligence(meta: Vec<Intelligence>, tool: Vec<Intelligence>) -> Vec<Intelligence> {
    let mut merged = meta;
    for ti in tool {
        if RESERVED_KINDS.contains(&ti.kind.as_str()) {
            eprintln!(
                "attach-meta: warning: tool intelligence kind '{}' shadows reserved meta kind — using meta version",
                ti.kind
            );
        } else {
            merged.push(ti);
        }
    }
    merged
}

fn scan_path_for_attachables(partial: Option<&str>) -> Vec<Suggestion> {
    let path_var = match std::env::var("PATH") {
        Ok(p) => p,
        Err(_) => return vec![],
    };

    let mut seen = std::collections::HashSet::new();
    let mut suggestions = Vec::new();
    let prefix = partial.unwrap_or("");

    for dir in path_var.split(':') {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let name = match entry.file_name().into_string() {
                Ok(n) => n,
                Err(_) => continue,
            };

            if !name.starts_with("attach-") {
                continue;
            }
            if name == "attach-meta" {
                continue;
            }
            if !prefix.is_empty() && !name.starts_with(prefix) {
                continue;
            }
            if !seen.insert(name.clone()) {
                continue;
            }
            if !is_executable(&entry.path()) {
                continue;
            }

            suggestions.push(Suggestion {
                value: name,
                display_string: None,
            });
        }
    }

    suggestions.sort_by(|a, b| a.value.cmp(&b.value));
    suggestions
}

fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(meta) => meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_intelligences_contains_attachable() {
        let intels = meta_intelligences();
        assert_eq!(intels.len(), 1);
        assert_eq!(intels[0].kind, "attachable");
    }

    #[test]
    fn is_meta_kind_checks_reserved() {
        assert!(is_meta_kind("attachable"));
        assert!(!is_meta_kind("device-key"));
        assert!(!is_meta_kind(""));
    }

    #[test]
    fn merge_no_collision() {
        let meta = meta_intelligences();
        let tool = vec![Intelligence {
            kind: "device-key".to_string(),
            args: vec![],
        }];
        let merged = merge_intelligence(meta, tool);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].kind, "attachable");
        assert_eq!(merged[1].kind, "device-key");
    }

    #[test]
    fn merge_collision_prefers_meta() {
        let meta = meta_intelligences();
        let tool = vec![
            Intelligence {
                kind: "attachable".to_string(),
                args: vec![IntelligenceArg {
                    name: "x".to_string(),
                    description: "tool version".to_string(),
                    required: false,
                    kind: None,
                }],
            },
            Intelligence {
                kind: "device-key".to_string(),
                args: vec![],
            },
        ];
        let merged = merge_intelligence(meta, tool);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].kind, "attachable");
        assert_eq!(merged[0].args.len(), 1);
        assert_eq!(merged[0].args[0].name, "partial");
        assert_eq!(merged[1].kind, "device-key");
    }

    #[test]
    fn meta_suggest_unknown_kind_errors() {
        assert!(meta_suggest("unknown", &[]).is_err());
    }
}
