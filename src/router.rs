use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{Result, bail};

use crate::cli::Command;
use crate::config::AppConfig;
use crate::discovery::{DiscoveryResponse, run_discovery};
use crate::schema::DiscoveryKey;

fn all_subcommands() -> Vec<&'static str> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for dk in DiscoveryKey::ALL {
        let w = dk.spec().argv[0];
        if seen.insert(w) {
            out.push(w);
        }
    }
    // Meta-commands that exist only in attach-meta, not in the protocol.
    for s in ["discover", "schema", "completions"] {
        out.push(s);
    }
    out
}

pub fn run_double_complete(
    current_word_index: usize,
    words: &[String],
    tool_override: Option<String>,
    config: AppConfig,
    verbose: bool,
) {
    macro_rules! trace {
        ($($arg:tt)*) => {
            if verbose { eprintln!("[attach-meta __complete] {}", format!($($arg)*)); }
        };
    }

    if words.len() <= 1 || current_word_index <= 1 {
        trace!("index {current_word_index} <= 1 — returning static subcommand list");
        for s in all_subcommands() {
            println!("{s}");
        }
        return;
    }

    // Strip binary name, then strip global attach-meta flags so the protocol verb
    // is always at protocol_words[0] regardless of where flags appear in the command line.
    let raw_words: Vec<&str> = words[1..].iter().map(String::as_str).collect();
    let raw_index = current_word_index - 1;
    let (protocol_words, protocol_index) = strip_global_flags(&raw_words, raw_index);

    if protocol_index == 0 {
        trace!("protocol_index 0 after flag-strip — returning static subcommand list");
        for s in all_subcommands() {
            println!("{s}");
        }
        return;
    }

    // Run discovery once; used for both sub-verb filtering and __complete delegation.
    let tool_info: Option<(String, DiscoveryResponse)> = {
        let tool_name = tool_override.as_deref().or(config.meta.default_tool.as_deref());
        if let Some(name) = tool_name {
            if let Some(tool) = config.find_tool(name) {
                match run_discovery(&tool.binary) {
                    Ok(d) => {
                        trace!(
                            "discovery ok — commands: {}",
                            d.commands.keys().cloned().collect::<Vec<_>>().join(", ")
                        );
                        Some((tool.binary.clone(), d))
                    }
                    Err(e) => {
                        trace!("discovery failed: {e}");
                        None
                    }
                }
            } else {
                trace!("tool '{name}' not found in config");
                None
            }
        } else {
            trace!("no tool configured — set default_tool in config or pass --tool");
            None
        }
    };

    // protocol_index == 1: completing the second protocol word (e.g. "create <TAB>").
    // Candidates come from the schema; filtered by discovery support when a tool is available.
    // Works even without a configured tool so new installs still get sub-verb hints.
    if protocol_index == 1 && !protocol_words.is_empty() {
        let verb0 = protocol_words[0];
        let partial = protocol_words.get(1).copied().unwrap_or("");
        let discovery_opt = tool_info.as_ref().map(|(_, d)| d);

        let sub_verbs: BTreeSet<&str> = DiscoveryKey::ALL
            .iter()
            .filter_map(|dk| {
                let spec = dk.spec();
                if spec.argv.len() < 2 || spec.argv[0] != verb0 {
                    return None;
                }
                if let Some(disc) = discovery_opt {
                    if !disc.commands.get(spec.key).map(|e| e.supported).unwrap_or(false) {
                        return None;
                    }
                }
                let candidate = spec.argv[1];
                if candidate.starts_with(partial) { Some(candidate) } else { None }
            })
            .collect();

        if !sub_verbs.is_empty() {
            trace!(
                "sub-verb completion: {verb0} {partial:?} → {}",
                sub_verbs.iter().cloned().collect::<Vec<_>>().join(", ")
            );
            for sv in sub_verbs {
                println!("{sv}");
            }
            return;
        }
        // No 2-word protocol entry matched — fall through to __complete (single-word verb)
    }

    let Some((binary, discovery)) = tool_info else {
        trace!("no tool/discovery available — cannot delegate to __complete");
        return;
    };

    match discovery.commands.get("__complete") {
        Some(e) if e.supported => {
            trace!("'__complete' command supported");
        }
        Some(_) => {
            trace!("'__complete' is listed but marked supported=false");
            return;
        }
        None => {
            trace!("'__complete' not found in discovery — tool does not implement completions");
            return;
        }
    }

    // Match protocol verb, translate to the tool's native argv.
    let best: Option<(DiscoveryKey, usize)> = DiscoveryKey::ALL
        .iter()
        .filter_map(|&dk| {
            let spec = dk.spec();
            let n = spec.argv.len();
            // Guard both: verb must not overlap the completing word, and slice must fit.
            if n > protocol_index || n > protocol_words.len() {
                return None;
            }
            let entry = discovery.commands.get(spec.key)?;
            if !entry.supported || entry.argv.is_empty() {
                return None;
            }
            if protocol_words[..n].iter().zip(spec.argv.iter()).all(|(a, b)| *a == *b) {
                Some((dk, n))
            } else {
                None
            }
        })
        .max_by_key(|&(_, n)| n);

    let (tool_words, adjusted_index): (Vec<&str>, usize) = match best {
        Some((dk, verb_count)) => {
            let spec = dk.spec();
            let native_argv = &discovery.commands[spec.key].argv;
            let native_len = native_argv.len();
            trace!(
                "resolved {:?} → {} (verb_count={verb_count}, native={native_argv:?})",
                &protocol_words[..verb_count], spec.key
            );
            let mut w: Vec<&str> = native_argv.iter().map(String::as_str).collect();
            w.extend_from_slice(&protocol_words[verb_count..]);
            (w, protocol_index + native_len - verb_count)
        }
        None => {
            trace!("no command matched {protocol_words:?} — passing through unchanged");
            (protocol_words, protocol_index)
        }
    };

    let mut argv: Vec<&str> = vec!["__complete", "--current-word-index"];
    let idx_str = adjusted_index.to_string();
    argv.push(&idx_str);
    argv.push("--");
    argv.extend_from_slice(&tool_words);
    trace!("calling: {} {}", binary, argv.join(" "));

    let output = match std::process::Command::new(&binary).args(&argv).output() {
        Ok(o) => o,
        Err(e) => {
            trace!("failed to run binary: {e}");
            return;
        }
    };

    trace!("exit status: {}", output.status);
    if !output.stderr.is_empty() {
        trace!("tool stderr: {}", String::from_utf8_lossy(&output.stderr).trim());
    }

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        trace!("{} candidates returned", stdout.lines().count());
        if verbose && stdout.is_empty() {
            eprintln!("[attach-meta __complete] stdout is empty — tool printed nothing");
        } else if verbose {
            for (i, line) in stdout.lines().enumerate() {
                eprintln!("[attach-meta __complete]   [{i}] {line:?}");
            }
        }
        print!("{stdout}");
    } else {
        trace!("tool exited non-zero — stdout: {:?}", String::from_utf8_lossy(&output.stdout));
    }
}

/// Strip global attach-meta flags from the word list, returning filtered words and adjusted index.
fn strip_global_flags<'a>(words: &[&'a str], index: usize) -> (Vec<&'a str>, usize) {
    const VALUE_FLAGS: &[&str] = &["--tool", "--workfile", "--config", "--setup-completions"];
    const BOOL_FLAGS: &[&str] = &["--json", "--verbose"];

    let mut out = Vec::with_capacity(words.len());
    let mut skipped_before = 0usize;
    let mut i = 0;

    while i < words.len() {
        let w = words[i];
        if VALUE_FLAGS.contains(&w) {
            if i < index { skipped_before += 1; }
            i += 1;
            if i < words.len() {
                if i < index { skipped_before += 1; }
                i += 1;
            }
        } else if BOOL_FLAGS.contains(&w) {
            if i < index { skipped_before += 1; }
            i += 1;
        } else {
            out.push(w);
            i += 1;
        }
    }

    (out, index.saturating_sub(skipped_before))
}

pub fn dispatch(
    command: Command,
    tool_override: Option<String>,
    workfile: Option<PathBuf>,
    json: bool,
    config: AppConfig,
) -> Result<()> {
    let tool_name = tool_override
        .as_deref()
        .or(config.meta.default_tool.as_deref())
        .ok_or_else(|| {
            anyhow::anyhow!("no tool specified — use --tool <name> or set default_tool in config")
        })?;

    let tool = config
        .find_tool(tool_name)
        .ok_or_else(|| anyhow::anyhow!("tool '{tool_name}' not found in config"))?;

    let binary = &tool.binary;

    // discover is a special case: pass through directly to the tool's discovery command
    if matches!(command, Command::Discover) {
        let mut extra = Vec::new();
        if json {
            extra.push("--json");
        }
        extra.push("discovery");
        let status = std::process::Command::new(binary)
            .args(&extra)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| anyhow::anyhow!("failed to run '{binary}': {e}"))?;
        std::process::exit(status.code().unwrap_or(1));
    }

    let discovery = run_discovery(binary)?;

    let key = command.key();
    let cmd_name = key.spec().key;
    let entry = discovery.commands.get(cmd_name).ok_or_else(|| {
        anyhow::anyhow!(
            "tool '{tool_name}' does not advertise a '{cmd_name}' command in its discovery output"
        )
    })?;

    if !entry.supported {
        bail!("tool '{tool_name}' does not support '{cmd_name}'");
    }

    let mut argv: Vec<&str> = Vec::new();

    if json {
        argv.push("--json");
    }

    argv.extend(entry.argv.iter().map(String::as_str));

    if let Some(wf) = &workfile {
        argv.push("--workfile");
        argv.push(wf.to_str().unwrap_or_default());
    }

    argv.extend(command.trailing_args().iter().map(String::as_str));

    let status = std::process::Command::new(binary)
        .args(&argv)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| anyhow::anyhow!("failed to run '{binary}': {e}"))?;

    std::process::exit(status.code().unwrap_or(1));
}
