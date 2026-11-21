use amml_api::{ApiError, Area, Client, Logger};

use crate::types::{FlagState, FlagWant};

/// The HUD and mouse flags of the game client, as the layers and the panel ask for them. The HUD is hidden while a
/// `Hud` lease with `hide_vanilla_hud` is active, and the player's own value is written back when it ends. The mouse
/// is released while the panel is open or a layer is in cursor mode, and grabbed again after, both only in play,
/// because the game handles the mouse itself in its menus. Each flag is written only when it changes.
///
/// A flag whose area the loader does not serve is off from the start. A write that gives `NotAvailable` turns off only
/// that flag, with one log line per flag; any other refusal (`NotReady` included) is logged once per flag, keeps the
/// flag as it was, and is tried again at the next tick, so a HUD the manager hid is shown again once the write lands.
/// Game thread.
pub struct ClientFlags {
    log: Logger,
    hud: FlagState,
    mouse: FlagState,
    // the player's hide-HUD value from before the manager hid the HUD
    player_hide: bool,
}

impl ClientFlags {
    const FEATURE: &'static str = "hud";

    /// Flags for a loader that serves `client.hud` when `hud` is true and `client.mouse` when `mouse` is true; a
    /// missing area is logged once here.
    pub fn new(log: Logger, hud: bool, mouse: bool) -> Self {
        let mut flags = Self { log, hud: FlagState::default(), mouse: FlagState::default(), player_hide: false };
        if !hud {
            flags.turn_off(Area::CLIENT_HUD, "area not served");
        }
        if !mouse {
            flags.turn_off(Area::CLIENT_MOUSE, "area not served");
        }
        flags
    }

    /// Writes the flags of `want` that changed since the last write.
    pub fn apply(&mut self, client: &Client<'_>, want: FlagWant) {
        if !self.hud.off && want.hide_hud != self.hud.applied {
            if want.hide_hud {
                self.player_hide = client.hide_hud().unwrap_or(false);
            }
            let value = if want.hide_hud { true } else { self.player_hide };
            match client.set_hide_hud(value) {
                Ok(()) => {
                    self.hud.applied = want.hide_hud;
                    self.log.info("hud hidden", &[("value", Self::text(value))]);
                }
                Err(error) => self.failed(Area::CLIENT_HUD, error),
            }
        }
        if !self.mouse.off && want.in_play && want.mouse_free != self.mouse.applied {
            match client.set_mouse_captured(!want.mouse_free) {
                Ok(()) => {
                    self.mouse.applied = want.mouse_free;
                    self.log.info("mouse captured", &[("value", Self::text(!want.mouse_free))]);
                }
                Err(error) => self.failed(Area::CLIENT_MOUSE, error),
            }
        }
    }

    /// Whether the game's own mouse grab must be cancelled: the mouse must stay free (`mouse_free`) and the manager
    /// can write the mouse flag.
    pub fn blocks_grab(&self, mouse_free: bool) -> bool {
        mouse_free && !self.mouse.off
    }

    /// The HUD flag is off (its area is not served or a write gave `NotAvailable`).
    pub fn hud_off(&self) -> bool {
        self.hud.off
    }

    /// The mouse flag is off, as `hud_off`.
    pub fn mouse_off(&self) -> bool {
        self.mouse.off
    }

    fn text(value: bool) -> &'static str {
        if value { "true" } else { "false" }
    }

    fn failed(&mut self, area: Area, error: ApiError) {
        match error {
            ApiError::NotAvailable => self.turn_off(area, &error.to_string()),
            _ => {
                let state = self.state_mut(area);
                if !state.refusal_logged {
                    state.refusal_logged = true;
                    self.log.warn(
                        "client flag refused",
                        &[("feature", Self::FEATURE), ("area", area.id), ("error", &error.to_string())],
                    );
                }
            }
        }
    }

    fn turn_off(&mut self, area: Area, reason: &str) {
        let state = self.state_mut(area);
        if state.off {
            return;
        }
        state.off = true;
        self.log.warn("feature off", &[("feature", Self::FEATURE), ("area", area.id), ("reason", reason)]);
    }

    fn state_mut(&mut self, area: Area) -> &mut FlagState {
        if area == Area::CLIENT_HUD { &mut self.hud } else { &mut self.mouse }
    }
}
