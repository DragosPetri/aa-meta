use std::path::Path;

use crate::config::AppConfig;
use crate::dynargs;
use crate::meta_intelligence;
use crate::protocol::manifest::{CommandName, Manifest};
use crate::transport;

pub fn run_complete(args: &[String], config: &mut AppConfig, config_path: &Path) {
    // Load manifest upfront — its command set drives which tool commands are available.
    let manifest: Option<Manifest> =
        crate::config::try_load_manifest(config, config_path).unwrap_or(None);

    // __complete -- <subcommand> [args...] <partial>
    // args is everything after "--"
    if args.is_empty() {
        print_command_list(manifest.as_ref());
        return;
    }

    let subcommand = &args[0];
    let rest = &args[1..];
    let partial = rest.last().map(|s| s.as_str()).unwrap_or("");

    // Handle init completions via meta-intelligence — only one positional accepted
    if subcommand == "init" {
        if rest.len() <= 1 {
            if let Ok(suggestions) = meta_intelligence::meta_suggest("attachable", &[]) {
                print_matching_suggestions(&suggestions, partial);
            }
        }
        return;
    }

    // Step 1: partial subcommand -> complete from manifest's declared command set
    let cmd = match CommandName::from_str(subcommand) {
        Some(c) => c,
        None => {
            if let Some(ref m) = manifest {
                for name in CommandName::ALL {
                    let s = name.as_str();
                    if s.starts_with(subcommand.as_str()) && m.commands.contains_key(s) {
                        println!("{s}");
                    }
                }
            }
            if "init".starts_with(subcommand.as_str()) {
                println!("init");
            }
            if "completion".starts_with(subcommand.as_str()) {
                println!("completion");
            }
            if "list-intelligence".starts_with(subcommand.as_str()) {
                println!("list-intelligence");
            }
            if "suggest".starts_with(subcommand.as_str()) {
                println!("suggest");
            }
            return;
        }
    };

    // Step 2: suggest subcommand
    if cmd == CommandName::Suggest {
        handle_suggest_completion(manifest.as_ref(), rest);
        return;
    }

    // list-intelligence has no completable args
    if cmd == CommandName::ListIntelligence {
        return;
    }

    // All other commands require a manifest
    let manifest = match manifest {
        Some(m) => m,
        None => return,
    };

    let mapping = match manifest.get_command(cmd) {
        Some(m) => m,
        None => return,
    };

    let array_flag_ctx = active_array_flag_ctx(
        &rest[..rest.len().saturating_sub(1)],
        cmd,
        mapping.args.as_ref(),
    );

    // Step 3: flag-name pass — suppressed when inside an array flag that has a completion entry
    // (unless partial itself starts with "--", meaning the user is explicitly asking for a flag)
    let preceding_flag = rest.len() >= 2
        && rest[rest.len() - 2].starts_with("--")
        && dynargs::flag_type_in_schema(
            rest[rest.len() - 2].trim_start_matches('-'),
            cmd,
            mapping.args.as_ref(),
        )
        .as_deref()
            != Some("boolean");

    let wants_flag = (partial.is_empty() || partial.starts_with("--"))
        && match &array_flag_ctx {
            Some((flag, _)) => {
                partial.starts_with("--")
                    || mapping
                        .completions
                        .as_ref()
                        .map_or(true, |cs| !cs.iter().any(|c| c.arg == flag.as_str()))
            }
            None => !preceding_flag,
        };

    if wants_flag {
        let flag_prefix = partial.strip_prefix("--").unwrap_or("");
        let already_used: Vec<&str> = rest.iter().filter_map(|a| a.strip_prefix("--")).collect();
        let all_flags = dynargs::collect_flag_names(cmd, mapping.args.as_ref());
        for flag in &all_flags {
            if flag.starts_with(flag_prefix) && !already_used.contains(&flag.as_str()) {
                println!("--{flag}");
            }
        }
    }

    // Step 4: value-completion pass via manifest completions entries
    let completions = match &mapping.completions {
        Some(c) => c,
        None => return,
    };

    // Determine completion kind and context for suggest.
    // Inside an array flag's values: use the flag's own completion entry and its preceding
    // values as context. Otherwise fall back to the existing flag-value / subcommand logic.
    let (completion_kind, suggest_context): (Option<&String>, Vec<String>) =
        if let Some((ref flag, ref preceding_values)) = array_flag_ctx {
            let kind = completions
                .iter()
                .find(|c| c.arg == flag.as_str())
                .map(|c| &c.kind);
            (kind, preceding_values.clone())
        } else {
            let preceding = if rest.len() >= 2 {
                Some(&rest[rest.len() - 2])
            } else {
                None
            };
            let kind = if let Some(prev) = preceding {
                if let Some(flag_name) = prev.strip_prefix("--") {
                    completions
                        .iter()
                        .find(|c| c.arg == flag_name)
                        .map(|c| &c.kind)
                } else {
                    completions
                        .iter()
                        .find(|c| c.arg == subcommand.as_str())
                        .map(|c| &c.kind)
                }
            } else {
                completions
                    .iter()
                    .find(|c| c.arg == subcommand.as_str())
                    .map(|c| &c.kind)
            };
            (
                kind,
                subcommand_positional_context(rest, cmd, mapping.args.as_ref()),
            )
        };

    let kind = match completion_kind {
        Some(k) => k,
        None => return,
    };

    // Steps 4-5: check if kind is advertised and call suggest
    // Meta kinds are handled locally
    if meta_intelligence::is_meta_kind(kind) {
        if let Ok(suggestions) = meta_intelligence::meta_suggest(kind, &suggest_context) {
            print_matching_suggestions(&suggestions, partial);
        }
        return;
    }

    let li_mapping = match manifest.get_command(CommandName::ListIntelligence) {
        Some(m) => m,
        None => return,
    };

    let li_response = match transport::invoke(li_mapping, &[]) {
        Ok(v) => v,
        Err(_) => return,
    };

    let intelligence = li_response.get("intelligence").and_then(|i| i.as_array());

    let advertised = intelligence.map_or(false, |intels| {
        intels
            .iter()
            .any(|i| i.get("kind").and_then(|k| k.as_str()) == Some(kind))
    });

    if !advertised {
        return;
    }

    let suggest_mapping = match manifest.get_command(CommandName::Suggest) {
        Some(m) => m,
        None => return,
    };

    let mut suggest_args = vec![kind.clone()];
    suggest_args.extend(suggest_context);

    let suggestions = match transport::invoke(suggest_mapping, &suggest_args) {
        Ok(v) => v,
        Err(_) => return,
    };

    if let Some(suggs) = suggestions.get("suggestions").and_then(|s| s.as_array()) {
        for s in suggs {
            if let Some(val) = s.get("value").and_then(|v| v.as_str()) {
                if val.starts_with(partial) {
                    println!("{val}");
                }
            }
        }
    }
}

fn handle_suggest_completion(manifest: Option<&Manifest>, rest: &[String]) {
    let meta = meta_intelligence::meta_intelligences();

    let tool_intelligence: Vec<crate::protocol::responses::Intelligence> = manifest
        .and_then(|m| m.get_command(CommandName::ListIntelligence))
        .and_then(|li_mapping| transport::invoke(li_mapping, &[]).ok())
        .and_then(|li_response| {
            serde_json::from_value(li_response.get("intelligence").cloned().unwrap_or_default())
                .ok()
        })
        .unwrap_or_default();

    let intelligence = meta_intelligence::merge_intelligence(meta, tool_intelligence);

    // Step 2a: completing the kind (first positional)
    if rest.is_empty() || (rest.len() == 1) {
        let partial = rest.first().map(|s| s.as_str()).unwrap_or("");
        for i in &intelligence {
            if i.kind.starts_with(partial) {
                println!("{}", i.kind);
            }
        }
        return;
    }

    // Step 2b: completing Nth argument
    let kind_str = &rest[0];
    let arg_index = rest.len() - 2; // 0-indexed after kind

    let intel = match intelligence.iter().find(|i| i.kind == *kind_str) {
        Some(i) => i,
        None => return,
    };

    let intel_arg = match intel.args.get(arg_index) {
        Some(a) => a,
        None => return,
    };

    let arg_kind = match &intel_arg.kind {
        Some(k) => k,
        None => return,
    };

    // Meta kinds are handled locally
    if meta_intelligence::is_meta_kind(arg_kind) {
        let partial = rest.last().map(|s| s.as_str()).unwrap_or("");
        let preceding: Vec<String> = rest[1..rest.len().saturating_sub(1)].to_vec();
        if let Ok(suggestions) = meta_intelligence::meta_suggest(arg_kind, &preceding) {
            print_matching_suggestions(&suggestions, partial);
        }
        return;
    }

    let advertised = intelligence.iter().any(|i| i.kind == *arg_kind);
    if !advertised {
        return;
    }

    let suggest_mapping = match manifest.and_then(|m| m.get_command(CommandName::Suggest)) {
        Some(m) => m,
        None => return,
    };

    let preceding: Vec<String> = rest[1..rest.len().saturating_sub(1)].to_vec();
    let mut suggest_args = vec![arg_kind.clone()];
    suggest_args.extend(preceding);

    let partial = rest.last().map(|s| s.as_str()).unwrap_or("");

    let suggestions = match transport::invoke(suggest_mapping, &suggest_args) {
        Ok(v) => v,
        Err(_) => return,
    };

    if let Some(suggs) = suggestions.get("suggestions").and_then(|s| s.as_array()) {
        for s in suggs {
            if let Some(val) = s.get("value").and_then(|v| v.as_str()) {
                if val.starts_with(partial) {
                    println!("{val}");
                }
            }
        }
    }
}

// Returns (flag_name, preceding_values) when `tokens` ends inside an array flag's value run:
// walk backwards — collect non-"--" tokens, then check if the nearest "--" flag is array type.
fn active_array_flag_ctx(
    tokens: &[String],
    cmd: CommandName,
    tool_args: Option<&serde_json::Value>,
) -> Option<(String, Vec<String>)> {
    let mut values: Vec<String> = Vec::new();
    for tok in tokens.iter().rev() {
        if let Some(flag_name) = tok.strip_prefix("--") {
            if dynargs::flag_type_in_schema(flag_name, cmd, tool_args).as_deref() == Some("array") {
                values.reverse();
                return Some((flag_name.to_string(), values));
            }
            return None;
        }
        values.push(tok.clone());
    }
    None
}

// Collect subcommand positional args from `rest`, excluding flag names and their values.
// Array flags skip all following tokens (greedy); string flags skip exactly one.
fn subcommand_positional_context(
    rest: &[String],
    cmd: CommandName,
    tool_args: Option<&serde_json::Value>,
) -> Vec<String> {
    let mut ctx: Vec<String> = Vec::new();
    enum Skip {
        None,
        One,
        UntilNextFlag,
    }
    let mut skip = Skip::None;
    for arg in rest.iter().take(rest.len().saturating_sub(1)) {
        match skip {
            Skip::One => {
                skip = Skip::None;
                continue;
            }
            Skip::UntilNextFlag => {
                if !arg.starts_with("--") {
                    continue;
                }
                skip = Skip::None;
            }
            Skip::None => {}
        }
        if let Some(flag_name) = arg.strip_prefix("--") {
            match dynargs::flag_type_in_schema(flag_name, cmd, tool_args).as_deref() {
                Some("array") => skip = Skip::UntilNextFlag,
                Some("boolean") => {}
                _ => skip = Skip::One,
            }
        } else {
            ctx.push(arg.clone());
        }
    }
    ctx
}

fn print_matching_suggestions(
    suggestions: &[crate::protocol::responses::Suggestion],
    partial: &str,
) {
    for s in suggestions {
        if s.value.starts_with(partial) {
            println!("{}", s.value);
        }
    }
}

fn print_command_list(manifest: Option<&Manifest>) {
    if let Some(m) = manifest {
        for cmd in CommandName::ALL {
            let s = cmd.as_str();
            if m.commands.contains_key(s) {
                println!("{s}");
            }
        }
    }
    println!("init");
    println!("completion");
    println!("list-intelligence");
    println!("suggest");
}

pub fn generate_completion_script(shell: &str) -> anyhow::Result<String> {
    match shell {
        "zsh" => Ok(generate_zsh_script()),
        "bash" => Ok(generate_bash_script()),
        "fish" => Ok(generate_fish_script()),
        other => anyhow::bail!("unsupported shell: {other}"),
    }
}

fn generate_zsh_script() -> String {
    r#"#compdef attach-meta
# Generated by attach-meta completion zsh
#
# To activate, ensure this file is on your fpath before compinit, e.g. add to ~/.zshrc:
#   fpath=(~/.zsh/completions $fpath)
#   autoload -Uz compinit && compinit

_attach-meta() {
    local -a completions
    completions=("${(@f)$(attach-meta __complete -- "${words[@]:1}" 2>/dev/null)}")
    if [[ $#completions -gt 0 ]]; then
        compadd -a completions
    fi
}

compdef _attach-meta attach-meta
"#
    .to_string()
}

fn generate_bash_script() -> String {
    r#"# Generated by attach-meta completion bash
#
# To activate, source this file from ~/.bashrc or ~/.bash_profile:
#   source /path/to/this/file
# Or run: attach-meta completion bash >> ~/.bashrc

_attach_meta_completions() {
    local IFS=$'\n'
    local completions
    completions=$(attach-meta __complete -- "${COMP_WORDS[@]:1}" 2>/dev/null) || return
    [ -z "$completions" ] && return
    while IFS= read -r value; do
        COMPREPLY+=("$value")
    done <<< "$completions"
}
complete -F _attach_meta_completions attach-meta
"#
    .to_string()
}

fn generate_fish_script() -> String {
    r#"# Generated by attach-meta completion fish
#
# To activate, place this file in ~/.config/fish/completions/attach-meta.fish
# Fish sources that directory automatically on startup.

function __attach_meta_completions
    set -l words (commandline -opc)
    set -l count (count $words)
    test $count -le 1; and return
    attach-meta __complete -- $words[2..-1] 2>/dev/null
end
complete -c attach-meta -f -a "(__attach_meta_completions)"
"#
    .to_string()
}
