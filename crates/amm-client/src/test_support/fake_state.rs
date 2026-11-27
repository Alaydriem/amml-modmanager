use std::collections::{HashMap, VecDeque};

use amm_abi::types::{AmmCallbacks, AmmEvent, AmmHandle, AmmRenderSize, AmmRenderSizeState, AmmStatus};
use amm_core::structs::descriptor::Capability;
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::manager::TextureGrant;
use amm_core::traits::AbiSized;

use super::SettingCall;

/// What the five fake manager tables recorded, and what they answer with. A test sets the answers before the call
/// and reads the records after it. The records grow with every call; a test makes few calls, so they have no bound.
pub struct FakeState {
    // The name of every table function called, in order
    pub calls: Vec<&'static str>,
    // Runs after each table function is recorded, with its name, and outside the state's borrow, so it may raise
    // loader events or call the link as a manager calling back would
    pub on_call: Option<Box<dyn FnMut(&'static str)>>,
    pub register_status: AmmStatus,
    pub handle: AmmHandle,
    pub descriptor: Option<String>,
    pub config_dir: Option<String>,
    pub callbacks: Option<AmmCallbacks>,
    pub callbacks_status: AmmStatus,
    pub unregistered: Vec<AmmHandle>,
    pub unregister_status: AmmStatus,
    pub events: VecDeque<AmmEvent>,
    pub config: String,
    // The answer to `acquire` per capability; a capability not in the map is granted
    pub acquire_outcomes: HashMap<Capability, AcquireOutcome>,
    pub acquired: Vec<Capability>,
    pub released: Vec<Capability>,
    pub set_setting_status: AmmStatus,
    pub settings: Vec<SettingCall>,
    pub holder_id: Option<&'static str>,
    // The answer to `held_by_other` per capability; a capability not in the map is not held by another mod
    pub held_by_other: HashMap<Capability, bool>,
    pub panel_open: bool,
    pub cursor_modes: Vec<bool>,
    pub texture_grant: TextureGrant,
    // (label, mark, screens) of each `register_menu_entry` call, and the id it answers with
    pub menu_entries: Vec<(String, String, u8)>,
    pub menu_entry_id: u16,
    // The language code `locale` answers with; empty is English
    pub locale: &'static str,
    // The length in bytes of each `set_icon` call, and the status it answers with
    pub icon_calls: Vec<usize>,
    pub icon_status: AmmStatus,
    // The status and the size `game_picture` answers
    pub game_picture: (AmmStatus, u32, u32),
    // Every rect `set_game_viewport` got, `None` for a clear
    pub game_viewports: Vec<Option<[f32; 4]>>,
    // Every size `set_render_size` got, `None` for an end, and the status it answers with
    pub render_sizes: Vec<Option<AmmRenderSize>>,
    pub render_size_status: AmmStatus,
    // The status and the state `render_size` answers
    pub render_size_state: (AmmStatus, AmmRenderSizeState),
}

impl FakeState {
    pub fn new() -> Self {
        Self {
            calls: Vec::new(),
            on_call: None,
            register_status: AmmStatus::Ok,
            handle: AmmHandle { slot: 3, generation: 7 },
            descriptor: None,
            config_dir: None,
            callbacks: None,
            callbacks_status: AmmStatus::Ok,
            unregistered: Vec::new(),
            unregister_status: AmmStatus::Ok,
            events: VecDeque::new(),
            config: String::new(),
            acquire_outcomes: HashMap::new(),
            acquired: Vec::new(),
            released: Vec::new(),
            set_setting_status: AmmStatus::Ok,
            settings: Vec::new(),
            holder_id: None,
            held_by_other: HashMap::new(),
            panel_open: false,
            cursor_modes: Vec::new(),
            texture_grant: TextureGrant { bytes: 0, count: 0 },
            menu_entries: Vec::new(),
            menu_entry_id: 1,
            locale: "",
            icon_calls: Vec::new(),
            icon_status: AmmStatus::Ok,
            game_picture: (AmmStatus::Ok, 0, 0),
            game_viewports: Vec::new(),
            render_sizes: Vec::new(),
            render_size_status: AmmStatus::Ok,
            render_size_state: (AmmStatus::Ok, AmmRenderSizeState::blank()),
        }
    }
}

impl Default for FakeState {
    fn default() -> Self {
        Self::new()
    }
}
