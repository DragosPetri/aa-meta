use crate::protocol::responses::Config;

pub trait Prompter {
    fn prompt_line(&self, prompt: &str) -> Option<String>;
    fn confirm(&self, prompt: &str, default: bool) -> bool;
}

pub struct StdinPrompter;

impl Prompter for StdinPrompter {
    fn prompt_line(&self, prompt: &str) -> Option<String> {
        eprint!("{prompt}");
        let mut buf = String::new();
        match std::io::stdin().read_line(&mut buf) {
            Ok(0) => None,
            Ok(_) => {
                let trimmed = buf.trim_end().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            }
            Err(_) => None,
        }
    }

    fn confirm(&self, prompt: &str, default: bool) -> bool {
        let suffix = if default { "[Y/n]" } else { "[y/N]" };
        match self.prompt_line(&format!("{prompt} {suffix} ")) {
            Some(s) => matches!(s.to_lowercase().as_str(), "y" | "yes"),
            None => default,
        }
    }
}

pub struct ScriptedPrompter {
    responses: std::cell::RefCell<Vec<String>>,
}

impl ScriptedPrompter {
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            responses: std::cell::RefCell::new(responses),
        }
    }
}

impl Prompter for ScriptedPrompter {
    fn prompt_line(&self, _prompt: &str) -> Option<String> {
        let mut r = self.responses.borrow_mut();
        if r.is_empty() {
            None
        } else {
            Some(r.remove(0))
        }
    }

    fn confirm(&self, _prompt: &str, default: bool) -> bool {
        match self.prompt_line("") {
            Some(s) => matches!(s.to_lowercase().as_str(), "y" | "yes"),
            None => default,
        }
    }
}

pub fn is_interactive() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

pub fn format_config_prompt(cfg: &Config) -> String {
    let required = if cfg.required { " (required)" } else { "" };
    let default_str = if cfg.default.is_null() {
        String::new()
    } else {
        format!(" [default: {}]", cfg.default)
    };
    format!(
        "  {}{}: {}{}",
        cfg.field_name, required, cfg.description, default_str
    )
}
