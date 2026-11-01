use crate::config::ResolutionBook;
use crate::services::lease::LeaseArbiter;
use crate::services::registry::ModRegistry;
use crate::structs::descriptor::Chord;
use crate::structs::registry::ModIndex;

/// What the router reads to place one event: panel state, the panel toggle chord, the mods whose
/// last frame asked for the pointer or the keyboard, whether the pointer is over the manager's
/// shown menu button, whether it is over the game viewport of the Overlay holder, and the registry and lease
/// state.
pub struct RouteContext<'a> {
    pub panel_open: bool,
    pub toggle_chord: Chord,
    pub pointer_owner: Option<ModIndex>,
    pub keyboard_owner: Option<ModIndex>,
    pub over_menu_button: bool,
    pub over_game_viewport: bool,
    pub registry: &'a ModRegistry,
    pub arbiter: &'a LeaseArbiter,
    pub book: &'a ResolutionBook,
}
