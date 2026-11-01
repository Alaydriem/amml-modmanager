use std::path::Path;

use amm_core::config::ManagerConfig;
use amm_core::services::ManagerService;
use amm_core::services::icon::IconDecoder;
use amm_core::structs::frame::FrameInputData;
use amm_core::structs::mesh::MeshBuffers;
use amm_core::structs::registry::ModIndex;
use amm_ui::model::PanelModel;
use amm_ui::runtime::PanelRuntime;
use amml_ui::font::FontSource;

use super::{Descriptors, IconPng};

/// A manager with four mods registered and a panel runtime with the panel open, run one pass at a time.
pub struct PanelRig {
    manager: ManagerService,
    mods: Vec<ModIndex>,
    runtime: PanelRuntime,
    out: MeshBuffers,
    time_secs: f64,
}

impl PanelRig {
    const ICON_SIDE: u32 = 256;
    const FRAME_STEP_SECS: f64 = 1.0 / 60.0;

    pub fn new() -> Self {
        let mut manager = ManagerService::new(ManagerConfig::default(), None);
        let mut mods = Vec::new();
        for (id, text) in [
            ("freecam", Descriptors::FREECAM),
            ("dejavu", Descriptors::DEJAVU),
            ("levels", Descriptors::LEVELS),
            ("minimap", Descriptors::MINIMAP),
        ] {
            let dir = std::env::temp_dir().join("amm-bench").join(id);
            let registered = manager.register(text, None, Path::new(&dir));
            mods.push(registered.unwrap_or_else(|error| panic!("{id} does not register: {error}")).0);
        }
        manager.set_panel_open(true);
        Self {
            manager,
            mods,
            runtime: PanelRuntime::new(FontSource::Embedded, ""),
            out: MeshBuffers::new(),
            time_secs: 0.0,
        }
    }

    /// Gives every mod a decoded 256 px icon.
    pub fn set_icons(&mut self) {
        self.set_icons_on(self.mods.len());
    }

    /// Gives the first `count` mods a decoded 256 px icon.
    pub fn set_icons_on(&mut self, count: usize) {
        let png = IconPng::square(Self::ICON_SIDE);
        for index in self.mods.iter().take(count).copied().collect::<Vec<_>>() {
            let icon = IconDecoder::decode(&png).unwrap_or_else(|error| panic!("the fixture icon decodes: {error}"));
            self.manager.set_icon(index, Some(icon)).unwrap_or_else(|error| panic!("the mod takes its icon: {error}"));
        }
    }

    /// Runs `frames` passes of the panel.
    pub fn run(&mut self, frames: u64) {
        for _ in 0..frames {
            self.time_secs += Self::FRAME_STEP_SECS;
            let input = FrameInputData {
                width: 1920,
                height: 1080,
                pixels_per_point: 1.0,
                time_secs: self.time_secs,
                events: &[],
                allowed_layers: FrameInputData::LAYER_OVERLAY,
                input_flags: FrameInputData::ALLOW_POINTER
                    | FrameInputData::ALLOW_KEYBOARD
                    | FrameInputData::ALLOW_NAV_KEYS,
            };
            let model = PanelModel::new(&self.manager, "1.4.1", 0);
            let _ = self.runtime.run(&input, &model, &mut self.out);
        }
    }
}

impl Default for PanelRig {
    fn default() -> Self {
        Self::new()
    }
}
