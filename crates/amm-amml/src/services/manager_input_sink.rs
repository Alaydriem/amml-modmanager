use amm_core::structs::input::{InputEvent, Modifiers as HostModifiers};
use amm_ffi::services::HostCell;

use crate::types::{FlagWant, PanelLayout};

/// The manager host as the input listeners see it: input events go in through `route`, and the panel and flag
/// state comes back from the lock-free host state. With no host every event passes and nothing is open.
#[derive(Clone, Copy)]
pub struct ManagerInputSink {
    cell: &'static HostCell,
}

impl ManagerInputSink {
    pub fn new(cell: &'static HostCell) -> Self {
        Self { cell }
    }

    /// The sink of the process host.
    pub fn global() -> Self {
        Self::new(HostCell::global())
    }

    /// Places one input event; true when the game must not see it. False with no host. It does not allocate.
    pub fn route(&self, event: InputEvent, now_ms: u64) -> bool {
        self.cell.with(|runtime| Ok(runtime.route(event, now_ms).consume)).unwrap_or(false)
    }

    /// Whether a key press another listener cancelled still goes to the host: only while the panel is open, or
    /// when it is the manager's own chord. A cancelled press never runs a key bind.
    pub fn takes_cancelled(&self, vk: u32, modifiers: HostModifiers, f3: bool) -> bool {
        self.cell
            .with(|runtime| {
                let toggle = runtime.manager().config().toggle_chord;
                Ok(runtime.panel_open() || modifiers.chord_for(vk).with_f3(f3) == toggle)
            })
            .unwrap_or(false)
    }

    /// Whether the panel is open. Lock-free.
    pub fn panel_open(&self) -> bool {
        self.cell.shared().is_some_and(|shared| shared.panel_open())
    }

    /// Where the open panel's controls are, from its last frame; `None` while it is closed or not drawn yet.
    pub fn panel_layout(&self) -> Option<PanelLayout> {
        let close = self.cell.with(|runtime| Ok(runtime.panel_close_rect())).ok().flatten()?;
        Some(PanelLayout { close, search: None })
    }

    /// The flags the layers and the panel ask for now, with `in_play` as given. Lock-free; all false with no host.
    pub fn wants(&self, in_play: bool) -> FlagWant {
        let Some(shared) = self.cell.shared() else {
            return FlagWant { in_play, ..FlagWant::default() };
        };
        let snapshot = shared.snapshot().load();
        FlagWant { hide_hud: snapshot.hide_vanilla_hud, mouse_free: snapshot.cursor_free, in_play }
    }

    /// The `Input` coexist pairs of the operator's resolutions, when they differ from `pushed`. It allocates only when
    /// the pairs changed.
    pub fn coexist_if_changed(&self, pushed: Option<&[(String, String)]>) -> Option<Vec<(String, String)>> {
        self.cell
            .with(|runtime| {
                let pairs = runtime.manager().arbiter().coexist_ids();
                Ok((pushed != Some(pairs)).then(|| pairs.to_vec()))
            })
            .ok()
            .flatten()
    }

    /// The input lease ranking of the operator's resolutions, when it differs from `pushed`. It allocates only when
    /// the ranking changed.
    pub fn ranking_if_changed(&self, pushed: Option<&[String]>) -> Option<Vec<String>> {
        self.cell
            .with(|runtime| {
                let ranking = runtime.manager().arbiter().ranking_ids();
                Ok((pushed != Some(ranking)).then(|| ranking.to_vec()))
            })
            .ok()
            .flatten()
    }
}
