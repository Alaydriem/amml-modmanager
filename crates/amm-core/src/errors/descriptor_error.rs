use crate::structs::descriptor::Capability;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DescriptorError {
    #[error("descriptor does not parse: {0}")]
    Parse(String),
    #[error("descriptor namespace or name is empty")]
    EmptyId,
    #[error("descriptor namespace and name must each be 1 to 64 characters of a-z, 0-9 and -: {0}")]
    BadId(String),
    #[error("config path must be a plain file name ending in .hcl: {0}")]
    BadConfigPath(String),
    #[error("{0} is a reserved name: a Windows device name or the manager's own id")]
    ReservedName(String),
    #[error("range, step or options of {path} are not valid")]
    BadKind { path: String },
    #[error("not a dotted path of identifiers: {0}")]
    BadPath(String),
    #[error("keybind {action} stores its chord at {path}, which is not a chord field")]
    KeybindPathNotChord { action: String, path: String },
    #[error("default of {path} is outside its range")]
    DefaultOutOfRange { path: String },
    #[error("default of {path} has the wrong type")]
    DefaultWrongType { path: String },
    #[error("two claims on {0:?}")]
    DuplicateClaim(Capability),
    #[error("two keybinds with action {0}")]
    DuplicateAction(String),
    #[error("two settings fields at {0}")]
    DuplicatePath(String),
    #[error("two sections with id {0}")]
    DuplicateSection(String),
    #[error("{0} keybinds is over the limit")]
    TooManyKeybinds(usize),
    #[error("{0} settings fields is over the limit")]
    TooManyFields(usize),
    #[error("hide_vanilla_hud is only valid on a Hud claim")]
    HideHudOnNonHudClaim,
    #[error("block_player_turn is only valid on an Input claim")]
    BlockTurnOnNonInputClaim,
    #[error("pass_pointer_motion is only valid on an Input claim")]
    PassMotionOnNonInputClaim,
    #[error("a RenderSize claim must be Exclusive: one mod forces the size at a time")]
    SharedRenderSize,
    #[error("{0} must be a whole number of at least 1")]
    ZeroTextureBudget(&'static str),
}
