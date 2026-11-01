use amm_core::structs::descriptor::Chord;
use amm_core::structs::input::Modifiers;

use crate::model::{PanelAction, PanelPage, PanelTab, RebindTarget};

/// The panel's own state between frames: the page and tab shown and a keybind waiting for its new
/// chord. It turns the operator's raw actions into the actions the host applies. Edits save on
/// their own, so closing never asks first.
#[derive(Debug, Default)]
pub struct PanelController {
    page: Option<PanelPage>,
    tab: PanelTab,
    rebinding: Option<RebindTarget>,
    // the modifiers held during a rebind, F3 included, one bit each in `MODIFIERS` order
    held: u8,
    // the modifier pressed with nothing else since; its release alone is a modifier-only chord
    solo: Option<u32>,
    // true once a second key joined a held modifier, until every modifier is released
    mixed: bool,
}

impl PanelController {
    const ESCAPE: u32 = 0x1B;
    const CONTROL: [u32; 3] = [0x11, 0xA2, 0xA3];
    const SHIFT: [u32; 3] = [0x10, 0xA0, 0xA1];
    const ALT: [u32; 3] = [0x12, 0xA4, 0xA5];
    const F3: [u32; 1] = [Chord::F3];
    const F3_BIT: u8 = 8;
    const MODIFIERS: [(u32, &'static str); 4] = [(0x11, "Ctrl"), (0x10, "Shift"), (0x12, "Alt"), (Chord::F3, "F3")];

    pub fn new() -> Self {
        Self::default()
    }

    /// The action for the host, or `None` when the panel keeps it. A page, tab or close drops a
    /// waiting rebind.
    pub fn on_action(&mut self, a: &PanelAction) -> Option<PanelAction> {
        match a {
            PanelAction::Select(target) => {
                self.page = Some(target.map_or(PanelPage::Conflicts, PanelPage::Mod));
                self.stop_rebind();
            }
            PanelAction::SelectTab(tab) => {
                self.tab = *tab;
                self.stop_rebind();
            }
            PanelAction::Close | PanelAction::Rebind { .. } | PanelAction::RebindToggle(_) => self.stop_rebind(),
            _ => {}
        }
        Some(a.clone())
    }

    /// Stops a waiting rebind.
    pub fn cancel(&mut self) {
        self.stop_rebind();
    }

    /// Forgets a waiting rebind, for a panel that closed outside `on_action`. The page and tab shown stay.
    pub fn reset(&mut self) {
        self.stop_rebind();
    }

    /// Waits for the next key press as the chord of a keybind.
    pub fn begin_rebind(&mut self, target: RebindTarget) {
        self.rebinding = Some(target);
        self.held = 0;
        self.solo = None;
        self.mixed = false;
    }

    /// Shows a page of the panel that is not a mod's own page.
    pub fn show_page(&mut self, page: PanelPage) {
        self.page = Some(page);
        self.stop_rebind();
    }

    /// A key event while the panel is open. During a rebind a modifier key down, F3 included, only joins
    /// the preview; the first other key down is the chord with the modifiers held then; a modifier other
    /// than F3 pressed and released with no other key between is its own chord; Escape cancels.
    pub fn on_key(&mut self, vk: u32, down: bool, mods: Modifiers) -> Option<PanelAction> {
        let target = self.rebinding?;
        if vk == Self::ESCAPE {
            if down {
                self.stop_rebind();
            }
            return None;
        }
        let chord = match (Self::modifier_bit(vk), down) {
            (Some((generic, bit)), true) => {
                let others = self.held & !bit != 0;
                let solo = !others && self.solo.is_none_or(|solo| solo == generic) && !self.mixed;
                self.mixed = !solo;
                self.solo = solo.then_some(generic);
                self.held |= bit;
                return None;
            }
            (Some((generic, bit)), false) => {
                self.held &= !bit;
                let alone = self.solo == Some(generic) && !self.mixed;
                self.solo = None;
                if self.held == 0 {
                    self.mixed = false;
                }
                if !alone || generic == Chord::F3 {
                    return None;
                }
                Modifiers::default().chord_for(generic)
            }
            (None, true) => self.held_mods(mods).chord_for(vk).with_f3(self.held & Self::F3_BIT != 0),
            (None, false) => return None,
        };
        self.stop_rebind();
        Some(match target {
            RebindTarget::Mod { mod_index, action } => PanelAction::Rebind { mod_index, action, chord },
            RebindTarget::ManagerToggle => PanelAction::RebindToggle(chord),
        })
    }

    /// The modifiers held during a rebind as the start of a chord, such as "Ctrl+Shift+...", or `None`
    /// while none is held.
    pub fn rebind_preview(&self) -> Option<String> {
        if self.rebinding.is_none() || self.held == 0 {
            return None;
        }
        let mut text = String::new();
        for (position, (_, name)) in Self::MODIFIERS.iter().enumerate() {
            if self.held & (1 << position) != 0 {
                text.push_str(name);
                text.push('+');
            }
        }
        text.push('\u{2026}');
        Some(text)
    }

    pub fn rebinding(&self) -> Option<RebindTarget> {
        self.rebinding
    }

    /// The page chosen by the operator, `None` before the first choice.
    pub fn page(&self) -> Option<PanelPage> {
        self.page
    }

    pub fn tab(&self) -> PanelTab {
        self.tab
    }

    fn stop_rebind(&mut self) {
        self.rebinding = None;
        self.held = 0;
        self.solo = None;
        self.mixed = false;
    }

    // The modifiers of a chord: those the event reports and those held since the rebind began.
    fn held_mods(&self, mods: Modifiers) -> Modifiers {
        Modifiers {
            ctrl: mods.ctrl || self.held & 1 != 0,
            shift: mods.shift || self.held & 2 != 0,
            alt: mods.alt || self.held & 4 != 0,
        }
    }

    // The one key a chord names for the left and right variants of a modifier, and its held bit.
    fn modifier_bit(vk: u32) -> Option<(u32, u8)> {
        [&Self::CONTROL[..], &Self::SHIFT, &Self::ALT, &Self::F3]
            .into_iter()
            .zip([1u8, 2, 4, Self::F3_BIT])
            .find(|(keys, _)| keys.contains(&vk))
            .map(|(keys, bit)| (keys[0], bit))
    }
}
