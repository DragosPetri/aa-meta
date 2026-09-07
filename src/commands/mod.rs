pub mod config;
pub mod crud;
pub mod init;
pub mod intelligence;
pub mod pipeline;
pub mod restructure;
pub mod validate;
pub mod workspace;

use crate::config::AppConfig;
use crate::error::AttachMetaError;
use crate::protocol::manifest::{CommandName, Manifest};

pub struct CommandContext {
    pub manifest: Manifest,
    pub tool_binary: String,
    pub json_output: bool,
    pub app_config: AppConfig,
    pub config_path: std::path::PathBuf,
}

pub fn dispatch(
    cmd: CommandName,
    positionals: &[String],
    flags: &serde_json::Value,
    ctx: &CommandContext,
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    match cmd {
        CommandName::ToolConfigGet | CommandName::ToolConfigSet => {
            config::run(cmd, positionals, flags, ctx)
        }
        CommandName::CreateWorkfile | CommandName::ListDevices => {
            workspace::run(cmd, positionals, flags, ctx)
        }
        CommandName::Add | CommandName::Read | CommandName::Update | CommandName::Delete => {
            crud::run(cmd, positionals, flags, ctx)
        }
        CommandName::Move | CommandName::Rename | CommandName::Alias => {
            restructure::run(cmd, positionals, flags, ctx)
        }
        CommandName::Validate => validate::run(cmd, positionals, flags, ctx),
        CommandName::Generate | CommandName::Build | CommandName::Deploy => {
            pipeline::run(cmd, positionals, flags, ctx)
        }
        CommandName::ListIntelligence | CommandName::Suggest => {
            intelligence::run(cmd, positionals, flags, ctx)
        }
    }
}
