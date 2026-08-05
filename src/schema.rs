pub struct CommandSpec {
    pub key: &'static str,
    pub argv: &'static [&'static str],
    pub description: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryKey {
    CreateNode,
    CreateProperty,
    CreateWorkfile,
    ReadNode,
    ReadProperty,
    Update,
    DeleteNode,
    DeleteProperty,
    Validate,
    Generate,
    Build,
    Deploy,
    Config,
    Init,
    ListDevices,
    Complete,
}

impl DiscoveryKey {
    pub fn spec(self) -> CommandSpec {
        match self {
            DiscoveryKey::CreateNode     => CommandSpec { key: "create_node",     argv: &["create", "node"],     description: "Add a new node" },
            DiscoveryKey::CreateProperty => CommandSpec { key: "create_property", argv: &["create", "property"], description: "Add a new property" },
            DiscoveryKey::CreateWorkfile => CommandSpec { key: "create_workfile", argv: &["create", "workfile"], description: "Create a new workfile" },
            DiscoveryKey::ReadNode       => CommandSpec { key: "read_node",       argv: &["read",   "node"],     description: "Read values of a node" },
            DiscoveryKey::ReadProperty   => CommandSpec { key: "read_property",   argv: &["read",   "property"], description: "Read values of a property" },
            DiscoveryKey::Update         => CommandSpec { key: "update",          argv: &["update"],             description: "Update primitive values" },
            DiscoveryKey::DeleteNode     => CommandSpec { key: "delete_node",     argv: &["delete", "node"],     description: "Delete a node" },
            DiscoveryKey::DeleteProperty => CommandSpec { key: "delete_property", argv: &["delete", "property"], description: "Delete a property" },
            DiscoveryKey::Validate       => CommandSpec { key: "validate",        argv: &["validate"],           description: "Validate workfile, node, or primitive" },
            DiscoveryKey::Generate       => CommandSpec { key: "generate",        argv: &["generate"],           description: "Generate an artifact from workfile" },
            DiscoveryKey::Build          => CommandSpec { key: "build",           argv: &["build"],              description: "Build from artifact" },
            DiscoveryKey::Deploy         => CommandSpec { key: "deploy",          argv: &["deploy"],             description: "Deploy built artifact to target" },
            DiscoveryKey::Config         => CommandSpec { key: "config",          argv: &["config"],             description: "Set a config value" },
            DiscoveryKey::Init          => CommandSpec { key: "init",            argv: &["init"],               description: "Initialize a new workfile or project" },
            DiscoveryKey::ListDevices   => CommandSpec { key: "list_devices",    argv: &["list-devices"],       description: "List available devices" },
            DiscoveryKey::Complete       => CommandSpec { key: "complete",        argv: &["complete"],           description: "Return completion candidates for a subcommand (optional)" },
        }
    }

    // Manual list used for documentation and schema output. Must stay in sync with
    // the enum variants above — a missing entry here means it's absent from
    // `attach-meta schema` output but does not affect dispatch correctness.
    pub const ALL: &'static [DiscoveryKey] = &[
        DiscoveryKey::CreateNode,
        DiscoveryKey::CreateProperty,
        DiscoveryKey::CreateWorkfile,
        DiscoveryKey::ReadNode,
        DiscoveryKey::ReadProperty,
        DiscoveryKey::Update,
        DiscoveryKey::DeleteNode,
        DiscoveryKey::DeleteProperty,
        DiscoveryKey::Validate,
        DiscoveryKey::Generate,
        DiscoveryKey::Build,
        DiscoveryKey::Deploy,
        DiscoveryKey::Config,
        DiscoveryKey::Init,
        DiscoveryKey::ListDevices,
        DiscoveryKey::Complete,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_keys_are_unique() {
        let mut seen = HashSet::new();
        for key in DiscoveryKey::ALL {
            let k = key.spec().key;
            assert!(seen.insert(k), "duplicate key in DiscoveryKey::ALL: {k}");
        }
    }
}
