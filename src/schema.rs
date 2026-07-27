pub struct CommandSpec {
    pub key: &'static str,
    pub argv: &'static [&'static str],
    pub description: &'static str,
}

pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec { key: "create_node",     argv: &["create", "node"],     description: "Add a new node" },
    CommandSpec { key: "create_property", argv: &["create", "property"], description: "Add a new property" },
    CommandSpec { key: "read_node",       argv: &["read",   "node"],     description: "Read values of a node" },
    CommandSpec { key: "read_property",   argv: &["read",   "property"], description: "Read values of a property" },
    CommandSpec { key: "update",          argv: &["update"],             description: "Update primitive values" },
    CommandSpec { key: "delete_node",     argv: &["delete", "node"],     description: "Delete a node" },
    CommandSpec { key: "delete_property", argv: &["delete", "property"], description: "Delete a property" },
    CommandSpec { key: "validate",        argv: &["validate"],           description: "Validate workfile, node, or primitive" },
    CommandSpec { key: "generate",        argv: &["generate"],           description: "Generate an artifact from workfile" },
    CommandSpec { key: "build",           argv: &["build"],              description: "Build from artifact" },
    CommandSpec { key: "deploy",          argv: &["deploy"],             description: "Deploy built artifact to target" },
    CommandSpec { key: "config",          argv: &["config"],             description: "Set a config value" },
    // optional — advertise to enable shell completions
    CommandSpec { key: "complete",        argv: &["complete"],           description: "Return completion candidates for a subcommand (optional)" },
];
