use super::{AMM_ABI_MAJOR, AMM_ABI_MINOR, AmmEvent, AmmHandle, AmmStatus};

/// The registry table `alaydriem.modmanager.core`: registration, events, the language, the panel state, menu
/// entries and the mod icon. New functions are only added at the end, with a higher minor. A null slot means the
/// function is absent. All text is UTF-8 as a pointer and a length. Game thread unless a function says otherwise.
/// After the manager is disabled every function answers `Disabled`, or 0 / nothing for one that gives no status.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AmmCoreTable {
    pub size: u32,
    pub version: u32,
    /// Registers a mod from its descriptor HCL text. `config_dir` is the mod's own AMML config folder as an absolute
    /// UTF-8 path with no `..` part, inside the AMML mods folder that holds the manager; its `config.hcl` (the
    /// descriptor's `config_path`) is read and written there. Any other folder, the manager's own config folder and
    /// the folder of another live mod are `InvalidArgument`. Both texts are valid only for the call. `out_handle`
    /// is written on success. Frame and `textures_lost` callbacks are given later with the layers table.
    pub register: Option<
        unsafe extern "C" fn(
            descriptor: *const u8,
            descriptor_len: usize,
            config_dir: *const u8,
            config_dir_len: usize,
            out_handle: *mut AmmHandle,
        ) -> AmmStatus,
    >,
    /// Releases everything the mod holds. It does not return while a `frame` or `textures_lost` callback of this
    /// mod runs on the render thread. On `Ok` no callback of this mod starts after it returns. Any other status
    /// means the registration may still be live (for example `NotPermitted` from inside one of this mod's
    /// callbacks). A handle that names no live registration is `UnknownHandle`.
    pub unregister: Option<unsafe extern "C" fn(handle: AmmHandle) -> AmmStatus>,
    /// Drains the mod event queue into `out_buf`, a buffer the caller owns with room for `capacity` events, and
    /// returns the number written. The manager keeps no pointer to the buffer after the call.
    pub poll_events: Option<unsafe extern "C" fn(handle: AmmHandle, out_buf: *mut AmmEvent, capacity: usize) -> usize>,
    /// The language code the person picked in the manager: empty for English, `en_XA` for the pseudo-locale, or a
    /// locale such as `ru` or `pt_BR`, copied into `out_buf` (room for `out_cap` bytes); returns its length. A
    /// return value greater than `out_cap` means the buffer was too small and nothing was written. `out_buf` may be
    /// null when `out_cap` is 0. A handle that names no live registration gives 0. Only a mod that has called it
    /// gets `LocaleChanged`.
    pub locale: Option<unsafe extern "C" fn(handle: AmmHandle, out_buf: *mut u8, out_cap: usize) -> u32>,
    /// Any thread. 1 while the manager panel is open, else 0.
    pub panel_open: Option<unsafe extern "C" fn() -> u8>,
    /// Places a button for the mod in the corner row of the start and pause screens: a tile with `mark` (1 to 3
    /// characters) and `label` (1 to 24 characters). `screens` is a bit set: 1 the start screen, 2 the pause screen;
    /// other bits are dropped, and no screen bit is `InvalidArgument`, as is a label or mark of the wrong length.
    /// On `Ok`, `out_id` gets the entry id, counted from 1 within the mod; a click sends the mod `MenuEntryClicked`
    /// with that id in `code`. A mod holds at most 4 entries and the manager 8, past either `QueueFull`. A null text
    /// or `out_id` is `NullPointer`. Both texts are valid only for the call.
    pub register_menu_entry: Option<
        unsafe extern "C" fn(
            handle: AmmHandle,
            label: *const u8,
            label_len: usize,
            mark: *const u8,
            mark_len: usize,
            screens: u8,
            out_id: *mut u16,
        ) -> AmmStatus,
    >,
    /// Sets the mod's icon in the manager panel from PNG file bytes: at most 262144 bytes, its first frame used,
    /// each side 1 to 256 pixels. A zero `png_len` clears the icon, and `png` may then be null. A null `png` with a
    /// `png_len` above 0 is `NullPointer`; bad bytes are `InvalidArgument` and the icon stays. `png` is valid only
    /// for the call.
    pub set_icon: Option<unsafe extern "C" fn(handle: AmmHandle, png: *const u8, png_len: usize) -> AmmStatus>,
}

// SAFETY: `AmmCoreTable` is `#[repr(C)]` and starts with `size: u32, version: u32`, and `size` is the byte size of
// the provider's table.
#[allow(unsafe_code)]
unsafe impl amml_api::Provided for AmmCoreTable {
    const NAME: &'static str = "alaydriem.modmanager.core";
    const MAJOR: u16 = AMM_ABI_MAJOR as u16;
    const MINOR: u16 = AMM_ABI_MINOR as u16;
}
