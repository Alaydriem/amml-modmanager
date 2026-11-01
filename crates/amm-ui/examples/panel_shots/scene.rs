use std::path::Path;

use amm_core::config::ManagerConfig;
use amm_core::errors::ManagerError;
use amm_core::services::manager::ManagerService;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::icon::ModIcon;
use amm_core::structs::input::{InputEvent, Modifiers};
use amm_core::structs::menu::MenuScreens;
use amm_core::structs::registry::ModIndex;

// The four fixture mods (minimap with its full settings schema) in a state that shows every part of
// the panel: a key clash, a suspension, held and blocked leases, edits waiting to save, a game change
// and two toasts
pub struct Scene {
    pub manager: ManagerService,
    pub minimap: ModIndex,
    pub freecam: ModIndex,
    pub freecam_cycle_mode: u16,
}

impl Scene {
    pub const QUIET_MS: u64 = 60_000;
    pub const TOAST_VIEW_MS: u64 = 4_200;
    const DENIED_MS: u64 = 1_000;
    const BLOCKED_MS: u64 = 3_500;
    const KEY_P: u32 = 0x50;
    const MINIMAP: &str = include_str!("../../fixtures/minimap_settings.hcl");
    const FREECAM: &str = include_str!("../../../amm-core/fixtures/four_mods/freecam.hcl");
    const LEVELS: &str = include_str!("../../../amm-core/fixtures/four_mods/levels.hcl");
    const DEJAVU: &str = include_str!("../../../amm-core/fixtures/four_mods/dejavu.hcl");

    pub fn new() -> Result<Self, String> {
        let mut manager = ManagerService::new(ManagerConfig::default(), None);
        let minimap = Self::register(&mut manager, Self::MINIMAP, "minimap")?;
        let freecam = Self::register(&mut manager, Self::FREECAM, "freecam")?;
        let levels = Self::register(&mut manager, Self::LEVELS, "levels")?;
        let dejavu = Self::register(&mut manager, Self::DEJAVU, "dejavu")?;
        let fail = |error: ManagerError| error.to_string();
        manager.set_setting(levels, "keys.toggle", "\"Ctrl+B\"", 0).map_err(fail)?;
        for cap in [Capability::Hud, Capability::Overlay] {
            manager.acquire(minimap, cap, 0).map_err(fail)?;
        }
        for cap in [Capability::Input, Capability::Hud, Capability::Overlay, Capability::WorldDraw] {
            manager.acquire(freecam, cap, 0).map_err(fail)?;
        }
        manager.acquire(dejavu, Capability::Camera, 0).map_err(fail)?;
        manager.register_menu_entry(dejavu, "Dejavu", "DV", MenuScreens::START).map_err(fail)?;
        manager.acquire(freecam, Capability::Camera, Self::DENIED_MS).map_err(fail)?;
        let ctrl = Modifiers { ctrl: true, shift: false, alt: false };
        manager.route(&InputEvent::Key { vk: Self::KEY_P, down: true, repeat: false, mods: ctrl }, Self::BLOCKED_MS);
        manager.route(&InputEvent::Key { vk: Self::KEY_P, down: false, repeat: false, mods: ctrl }, Self::BLOCKED_MS);
        manager.edit_draft(minimap, "map.zoom", hcl::Value::from(3.0), 0).map_err(fail)?;
        manager.set_setting(minimap, "map.zoom", "2.5", 0).map_err(fail)?;
        manager.edit_draft(minimap, "map.radius", hcl::Value::from(12), 0).map_err(fail)?;
        manager.edit_draft(minimap, "map.shape", hcl::Value::from("circle"), 0).map_err(fail)?;
        manager.set_icon(freecam, Some(Self::icon())).map_err(fail)?;
        manager.set_panel_open(true);
        let freecam_cycle_mode = Self::action(&manager, freecam, "cycle_mode")?;
        Ok(Self { manager, minimap, freecam, freecam_cycle_mode })
    }

    // a 64 by 64 diagonal gradient with opaque corners
    fn icon() -> ModIcon {
        let side: u16 = 64;
        let mut rgba = Vec::with_capacity(usize::from(side) * usize::from(side) * 4);
        for y in 0..side {
            for x in 0..side {
                let (x, y) = (u8::try_from(x * 4).unwrap_or(u8::MAX), u8::try_from(y * 4).unwrap_or(u8::MAX));
                rgba.extend_from_slice(&[x, 120, y, 255]);
            }
        }
        ModIcon { width: side, height: side, rgba }
    }

    fn register(manager: &mut ManagerService, hcl: &str, id: &str) -> Result<ModIndex, String> {
        let dir = std::env::temp_dir().join("amm-ui-panel-shots").join(id);
        manager.register(hcl, None, Path::new(&dir)).map(|(index, _)| index).map_err(|error| format!("{id}: {error}"))
    }

    fn action(manager: &ManagerService, index: ModIndex, name: &str) -> Result<u16, String> {
        let registered = manager.registry().get(index).ok_or("freecam is not registered")?;
        let position = registered.descriptor.keybinds.iter().position(|bind| bind.action == name);
        position.and_then(|position| u16::try_from(position).ok()).ok_or_else(|| format!("no keybind {name}"))
    }
}
