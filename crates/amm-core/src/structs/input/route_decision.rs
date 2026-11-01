use crate::structs::event::KeybindPhase;
use crate::structs::registry::{ModIndex, ModMask};
use crate::structs::toast::ToastKind;

/// Where one input event goes. Plain data: the caller cancels the game event, toggles or feeds the
/// panel, queues the keybind and raw events, and shows the toast.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RouteDecision {
    pub consume_from_game: bool,
    pub toggle_panel: bool,
    pub to_panel: bool,
    pub keybind: Option<(ModIndex, u16, KeybindPhase)>,
    pub raw_targets: ModMask,
    pub blocked: Option<ToastKind>,
}
