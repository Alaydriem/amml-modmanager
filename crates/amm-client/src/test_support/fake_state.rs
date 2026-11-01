use std::collections::{HashMap, VecDeque};

use amm_abi::types::{
    AmmCallbacks, AmmCaptureStreamFrame, AmmEvent, AmmHandle, AmmRenderSize, AmmRenderSizeState, AmmStatus,
};
use amm_core::structs::descriptor::Capability;
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::manager::TextureGrant;
use amm_core::traits::AbiSized;

use super::{LoggedCall, SettingCall};

/// What the fake table recorded, and what it answers with. A test sets the answers before the call and reads
/// the records after it. The records grow with every call; a test makes few calls, so they have no bound.
#[derive(Debug)]
pub struct FakeState {
    pub register_status: AmmStatus,
    pub handle: AmmHandle,
    pub descriptor: Option<String>,
    pub config_dir: Option<String>,
    pub callbacks: Option<AmmCallbacks>,
    pub unregistered: Vec<AmmHandle>,
    pub unregister_status: AmmStatus,
    pub holder_id: Option<&'static str>,
    pub events: VecDeque<AmmEvent>,
    pub config: String,
    // The answer to `acquire` per capability; a capability not in the map is granted
    pub acquire_outcomes: HashMap<Capability, AcquireOutcome>,
    pub acquired: Vec<Capability>,
    pub released: Vec<Capability>,
    pub set_setting_status: AmmStatus,
    pub settings: Vec<SettingCall>,
    pub logs: Vec<LoggedCall>,
    // The status and sequence `request_screenshot` answers with
    pub screenshot_status: AmmStatus,
    pub screenshot_sequence: u32,
    // The path text of each `request_screenshot` call
    pub screenshot_requests: Vec<String>,
    // The `layers` byte of each `request_screenshot_layers` call, whose path is also in `screenshot_requests`
    pub screenshot_layers: Vec<u8>,
    // The answer to `held_by_other` per capability; a capability not in the map is not held by another mod
    pub held_by_other: HashMap<Capability, bool>,
    pub texture_grant: TextureGrant,
    // (label, mark, screens) of each `register_menu_entry` call, and the id it answers with
    pub menu_entries: Vec<(String, String, u8)>,
    pub menu_entry_id: u16,
    // The language code `locale` answers with; empty is English. `locale_reads` counts the calls
    pub locale: &'static str,
    pub locale_reads: u32,
    // The length in bytes of each `set_icon` call, and the status it answers with
    pub icon_calls: Vec<usize>,
    pub icon_status: AmmStatus,
    // the status and the size `game_picture` answers
    pub game_picture: (AmmStatus, u32, u32),
    // every rect `set_game_viewport` got, `None` for a clear
    pub game_viewports: Vec<Option<[f32; 4]>>,
    // every size `set_render_size` got, `None` for an end, and the status it answers with
    pub render_sizes: Vec<Option<AmmRenderSize>>,
    pub render_size_status: AmmStatus,
    // the status and the state `render_size` answers
    pub render_size_state: (AmmStatus, AmmRenderSizeState),
    // the tag of each `capture_stream_request` call, and the status it answers with
    pub stream_requests: Vec<u64>,
    // the `present_seq` of each `capture_stream_request` call
    pub stream_present_seqs: Vec<u64>,
    pub stream_request_status: AmmStatus,
    // what each `capture_stream_take` answers, in turn: the status, the frame and its pixels; `Busy` once empty
    pub stream_takes: VecDeque<(AmmStatus, AmmCaptureStreamFrame, Vec<u8>)>,
    // the pixels of the last frame taken, lent until the next take
    pub stream_lent: Vec<u8>,
    // the pixel buffers `capture_stream_lend` lent, kept for the test; a Vec never moves its heap bytes
    pub stream_lent_buffers: Vec<Vec<u8>>,
    // the lend id of each `capture_stream_give_back` call, and the status it answers with
    pub stream_give_backs: Vec<u64>,
    pub stream_give_back_status: AmmStatus,
    // the count `capture_stream_free` answers
    pub stream_free: u32,
    // the `capture_stream_close` calls
    pub stream_closes: u32,
    // the status and the count `present_count` answers
    pub present_count: (AmmStatus, u64),
    // the key set of the last `watch_keys` call, and the status it answers with
    pub watched_keys: Vec<u32>,
    pub watch_keys_status: AmmStatus,
    // the (vk, down) of each `send_key` call, and the status it answers with
    pub sent_key_calls: Vec<(u32, bool)>,
    pub send_key_status: AmmStatus,
}

impl FakeState {
    pub fn new() -> Self {
        Self {
            register_status: AmmStatus::Ok,
            handle: AmmHandle { slot: 3, generation: 7 },
            descriptor: None,
            config_dir: None,
            callbacks: None,
            unregistered: Vec::new(),
            unregister_status: AmmStatus::Ok,
            holder_id: None,
            events: VecDeque::new(),
            config: String::new(),
            acquire_outcomes: HashMap::new(),
            acquired: Vec::new(),
            released: Vec::new(),
            set_setting_status: AmmStatus::Ok,
            settings: Vec::new(),
            logs: Vec::new(),
            screenshot_status: AmmStatus::Ok,
            screenshot_sequence: 1,
            screenshot_requests: Vec::new(),
            screenshot_layers: Vec::new(),
            held_by_other: HashMap::new(),
            texture_grant: TextureGrant { bytes: 0, count: 0 },
            menu_entries: Vec::new(),
            menu_entry_id: 1,
            locale: "",
            locale_reads: 0,
            icon_calls: Vec::new(),
            icon_status: AmmStatus::Ok,
            game_picture: (AmmStatus::Ok, 0, 0),
            game_viewports: Vec::new(),
            render_sizes: Vec::new(),
            render_size_status: AmmStatus::Ok,
            render_size_state: (AmmStatus::Ok, AmmRenderSizeState::blank()),
            stream_requests: Vec::new(),
            stream_present_seqs: Vec::new(),
            stream_request_status: AmmStatus::Ok,
            stream_takes: VecDeque::new(),
            stream_lent: Vec::new(),
            stream_lent_buffers: Vec::new(),
            stream_give_backs: Vec::new(),
            stream_give_back_status: AmmStatus::Ok,
            stream_free: 0,
            stream_closes: 0,
            present_count: (AmmStatus::Ok, 0),
            watched_keys: Vec::new(),
            watch_keys_status: AmmStatus::Ok,
            sent_key_calls: Vec::new(),
            send_key_status: AmmStatus::Ok,
        }
    }

    /// The key set the last `watch_keys` call passed; empty before any call or after a clear.
    pub fn watched(&self) -> Vec<u32> {
        self.watched_keys.clone()
    }

    /// The `(vk, down)` of each `send_key` call, in order.
    pub fn sent_keys(&self) -> Vec<(u32, bool)> {
        self.sent_key_calls.clone()
    }

    /// True when a mod registered and gave neither a frame nor a textures-lost callback.
    pub fn registered_without_frame(&self) -> bool {
        self.callbacks.is_some_and(|callbacks| callbacks.frame.is_none() && callbacks.textures_lost.is_none())
    }
}

impl Default for FakeState {
    fn default() -> Self {
        Self::new()
    }
}
