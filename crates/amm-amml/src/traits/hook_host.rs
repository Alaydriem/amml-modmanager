use amml_sdk::{HookHandle, SdkError};

use crate::types::HookSpec;

/// What installing the game hooks needs from the loader: entry values, hooks and one log line per lost feature.
/// `AmmlHookHost` wraps the AMML `Api`; tests fake it.
pub trait HookHost {
    fn resolve(&self, id: &str) -> Result<u64, SdkError>;

    /// Installs `spec.detour` on `target`, the resolved value of `spec.entry`.
    fn hook(&self, target: u64, spec: &HookSpec) -> Result<HookHandle, SdkError>;

    fn unhook(&self, hook: HookHandle) -> Result<(), SdkError>;

    fn warn(&self, message: &str);
}
