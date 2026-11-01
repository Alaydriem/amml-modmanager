use amm_core::config::ManagerConfig;
use amm_core::services::manager::ManagerService;
use amm_core::services::settings::ChordText;
use amm_core::structs::clash::{ClashKind, ClashReport};
use amm_core::structs::descriptor::{Capability, Chord, ClaimMode, KeyScope, KeybindDecl, SettingField};
use amm_core::structs::icon::IconEntry;
use amm_core::structs::menu::MenuEntry;
use amm_core::structs::registry::{ModIndex, RegisteredMod};
use amm_core::structs::settings::SettingsDraft;
use amm_core::structs::toast::Toast;
use amm_core::structs::write::SaveStatus;

use crate::model::{KeybindRow, LeaseState, RebindTarget};

/// The read-only view of the manager that one panel pass draws from: the registered mods with
/// their descriptors and lease states, the settings drafts, the save status, the keybinds, the clash
/// report, the toasts and the manager version, and whether the menu button shows. The host builds it on each frame that draws the
/// panel, a toast or the menu button.
#[derive(Clone, Copy)]
pub struct PanelModel<'a> {
    manager: &'a ManagerService,
    manager_version: &'a str,
    now_ms: u64,
    panel_open: bool,
    menu_button: bool,
}

impl<'a> PanelModel<'a> {
    /// How long a toast stays on screen.
    pub const TOAST_TTL_MS: u64 = 5000;

    pub fn new(manager: &'a ManagerService, manager_version: &'a str, now_ms: u64) -> Self {
        let panel_open = manager.snapshot().panel_open;
        let menu_button = manager.menu_button_visible();
        Self { manager, manager_version, now_ms, panel_open, menu_button }
    }

    pub fn manager_version(&self) -> &'a str {
        self.manager_version
    }

    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }

    pub fn panel_open(&self) -> bool {
        self.panel_open
    }

    /// True while the start or pause screen is on top, the config allows the button and the panel is closed.
    pub fn menu_button_visible(&self) -> bool {
        self.menu_button
    }

    /// The menu entries mods placed for the screen on top, in registration order.
    pub fn menu_entries(&self) -> impl Iterator<Item = &'a MenuEntry> + 'a {
        self.manager.menu_entries().for_screen(self.manager.menu_screen())
    }

    pub fn config(&self) -> &'a ManagerConfig {
        self.manager.config()
    }

    pub fn mods(&self) -> impl Iterator<Item = &'a RegisteredMod> + 'a {
        self.manager.registry().iter()
    }

    pub fn mod_count(&self) -> usize {
        self.manager.registry().len()
    }

    pub fn enabled_count(&self) -> usize {
        self.mods().filter(|registered| registered.enabled).count()
    }

    pub fn first_mod(&self) -> Option<ModIndex> {
        self.mods().next().map(|registered| registered.index)
    }

    pub fn registered(&self, index: ModIndex) -> Option<&'a RegisteredMod> {
        self.manager.registry().get(index)
    }

    /// The manager's language code: empty for English. It may name a language with no pack. Spaces
    /// around it are dropped, as `LocaleSetting::from_code` drops them.
    pub fn language(&self) -> &'a str {
        self.manager.locale().trim()
    }

    /// The icon the mod in `index` set, if any.
    pub fn icon(&self, index: ModIndex) -> Option<&'a IconEntry> {
        self.manager.icon(index)
    }

    pub fn keybind(&self, index: ModIndex, action: u16) -> Option<&'a KeybindDecl> {
        self.registered(index)?.descriptor.keybinds.get(usize::from(action))
    }

    pub fn draft(&self, index: ModIndex) -> Option<&'a SettingsDraft> {
        self.manager.draft().get(usize::from(index.0))
    }

    /// What the header says about saving now.
    pub fn save_status(&self) -> SaveStatus {
        self.manager.save_status(self.now_ms)
    }

    /// Every keybind as the all-keybinds page lists it: the panel toggle first, then each mod's binds
    /// in registration and declaration order.
    pub fn keybind_rows(&self) -> Vec<KeybindRow<'a>> {
        let mut rows = vec![self.toggle_row()];
        for registered in self.mods() {
            let index = registered.index;
            for (position, bind) in registered.descriptor.keybinds.iter().enumerate() {
                let Ok(action) = u16::try_from(position) else {
                    break;
                };
                let exclusive = registered
                    .descriptor
                    .claims
                    .iter()
                    .any(|claim| claim.capability == bind.capability && claim.mode == ClaimMode::Exclusive);
                rows.push(KeybindRow {
                    target: RebindTarget::Mod { mod_index: index, action },
                    mod_name: registered.descriptor.display_name.as_str(),
                    label: bind.label.as_str(),
                    chord: self.chord(index, bind),
                    capability: Some(bind.capability),
                    scope: bind.scope,
                    exclusive,
                    clash_with: self.key_clash(index, action),
                });
            }
        }
        rows[0].clash_with = rows[1..].iter().find(|row| row.chord == rows[0].chord).and_then(|row| match row.target {
            RebindTarget::Mod { mod_index, .. } => Some(mod_index),
            RebindTarget::ManagerToggle => None,
        });
        rows
    }

    /// The other mod of a key clash on a mod's keybind.
    pub fn key_clash(&self, index: ModIndex, action: u16) -> Option<ModIndex> {
        self.clash_report().conflicts.iter().find_map(|clash| match clash.kind {
            ClashKind::Key { a, b, .. } if a.mod_index == index && a.action == action => Some(b.mod_index),
            ClashKind::Key { a, b, .. } if b.mod_index == index && b.action == action => Some(a.mod_index),
            _ => None,
        })
    }

    fn toggle_row(&self) -> KeybindRow<'a> {
        KeybindRow {
            target: RebindTarget::ManagerToggle,
            // the view names the toggle's row and its group from messages
            mod_name: "",
            label: "",
            chord: self.config().toggle_chord,
            capability: None,
            scope: KeyScope::Global,
            exclusive: false,
            clash_with: None,
        }
    }

    /// The draft value of a field, or its default when the draft has none.
    pub fn field_value(&self, index: ModIndex, field: &SettingField) -> hcl::Value {
        self.draft(index).and_then(|draft| draft.value(&field.path)).unwrap_or_else(|| field.default.clone())
    }

    /// The chord a keybind has in the draft, or its default when the draft value does not parse.
    pub fn chord(&self, index: ModIndex, bind: &KeybindDecl) -> Chord {
        self.draft(index)
            .and_then(|draft| draft.value(&bind.config_path))
            .and_then(|value| value.as_str().and_then(ChordText::parse))
            .unwrap_or(bind.default_chord)
    }

    pub fn clash_report(&self) -> &'a ClashReport {
        self.manager.clash_report()
    }

    /// The toasts on screen now, oldest first.
    pub fn toasts(&self) -> impl Iterator<Item = &'a Toast> + 'a {
        self.manager.toasts().visible(self.now_ms, Self::TOAST_TTL_MS)
    }

    pub fn has_visible_toasts(&self) -> bool {
        self.toasts().next().is_some()
    }

    pub fn lease_state(&self, index: ModIndex, cap: Capability) -> LeaseState {
        let claimed = self
            .registered(index)
            .is_some_and(|registered| registered.descriptor.claims.iter().any(|claim| claim.capability == cap));
        if !claimed {
            return LeaseState::NotClaimed;
        }
        let arbiter = self.manager.arbiter();
        if arbiter.holds(index, cap) {
            return LeaseState::Held;
        }
        if arbiter.is_suspended(index, cap)
            && let Some(by) = arbiter.exclusive_holder(cap)
        {
            return LeaseState::Suspended { by };
        }
        match arbiter.blocked(index, cap) {
            Some(by) => LeaseState::Blocked { by },
            None => LeaseState::Idle,
        }
    }
}
