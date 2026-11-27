use std::path::{Component, Path};

/// The checks a path from a mod must pass before the manager uses it. Each check returns why the path is
/// refused, or `None`.
pub struct PathRules;

impl PathRules {
    /// Why a folder or file path is refused on its own text, or `None`. It must be absolute with no `..` part.
    /// A path that starts with two separators is a network share (`\\server\`), a device path (`\\.\`) or a
    /// verbatim path (`\\?\`), and is refused.
    pub fn fault(path: &Path) -> Option<&'static str> {
        let text = path.to_string_lossy();
        let remote = text.len() >= 2 && text.chars().take(2).all(|ch| ch == '\\' || ch == '/');
        if remote {
            return Some("it is a network, device or verbatim path");
        }
        let has_parent = path.components().any(|part| part == Component::ParentDir);
        (text.is_empty() || !path.is_absolute() || has_parent).then_some("it must be absolute with no ..")
    }

    /// The AMML mods folder that holds the manager, from the manager's own config folder
    /// `<mods>/@<namespace>/<name>/config`: the folder three levels up. `None` for a path with fewer levels.
    pub fn mods_root(manager_config_dir: &Path) -> Option<&Path> {
        manager_config_dir.ancestors().nth(3).filter(|root| !root.as_os_str().is_empty())
    }
}
