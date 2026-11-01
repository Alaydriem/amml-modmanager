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

    /// Why a screenshot file path is refused on its own text, or `None`. On top of `fault`, it must be UTF-8 and
    /// end in `.png` in any case. It does not look at the file system: the host checks that the parent folder
    /// exists.
    pub fn screenshot_fault(path: &Path) -> Option<&'static str> {
        if let Some(reason) = Self::fault(path) {
            return Some(reason);
        }
        if path.to_str().is_none() {
            return Some("it is not UTF-8");
        }
        let is_png = path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("png"));
        (!is_png).then_some("it does not end in .png")
    }
}
