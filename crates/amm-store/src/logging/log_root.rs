use std::path::{Path, PathBuf};

/// `<game>/logs/amm`, the folder that holds one sub-folder of session files per mod.
#[derive(Debug, Clone)]
pub struct LogRoot {
    root: PathBuf,
}

impl LogRoot {
    pub fn new(game_dir: &Path) -> LogRoot {
        Self { root: game_dir.join("logs").join("amm") }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folder of one mod, `<root>/@<namespace>/<name>`. In each part, characters outside letters, digits, `-`
    /// and `_` become `_`, so a part cannot name a path outside the root.
    pub fn dir_for(&self, namespace: &str, name: &str) -> PathBuf {
        self.root.join(format!("@{}", Self::segment(namespace))).join(Self::segment(name))
    }

    fn segment(part: &str) -> String {
        let clean: String =
            part.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
        if clean.is_empty() { "_".to_string() } else { clean }
    }
}
