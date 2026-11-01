use std::path::PathBuf;

use crate::structs::manager::TextureGrant;
use crate::structs::settings::HclDocument;

/// What the manager keeps for one registered mod beside its registry entry: where its config file
/// is, the saved document, when it registered, its frame failures in a row, and whether a saved
/// `WorldReload` change waits for the next world load, and its texture budget. A mod whose config
/// file could not be read or kept has `write_back` false: its file is never written.
#[derive(Debug, Clone)]
pub struct ManagedMod {
    pub config_path: PathBuf,
    pub document: HclDocument,
    pub registered_seq: u64,
    pub failures: u32,
    pub world_reload_pending: bool,
    pub write_back: bool,
    // when a refused write last showed its toast and log line, for the per-mod rate limit
    pub refusal_shown_ms: Option<u64>,
    pub texture_grant: TextureGrant,
}

impl ManagedMod {
    pub fn new(
        config_path: PathBuf,
        document: HclDocument,
        registered_seq: u64,
        write_back: bool,
        texture_grant: TextureGrant,
    ) -> Self {
        Self {
            config_path,
            document,
            registered_seq,
            failures: 0,
            world_reload_pending: false,
            write_back,
            refusal_shown_ms: None,
            texture_grant,
        }
    }
}
