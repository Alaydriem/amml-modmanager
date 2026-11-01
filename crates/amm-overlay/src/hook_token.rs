/// The handle of one installed hook, as the `HookInstaller` that installed it gave it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookToken(pub u64);
