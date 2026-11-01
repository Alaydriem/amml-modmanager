use amm_core::traits::AbiSized;

use super::{
    AmmAcquireResult, AmmCallbacks, AmmCaptureStreamFrame, AmmCaptureStreamRequest, AmmEvent, AmmHandle, AmmRenderSize,
    AmmRenderSizeState, AmmStatus,
};

/// The function table returned by `amm_get_api`. New functions are only added at the end, with a higher `abi_minor`.
/// A client checks `size` before it calls a function added after its own minor version. The functions from
/// `register` to `capture_stream_give_back` are part of 2.0; a function after it came in the later minor version
/// its doc names (`watch_keys` and `send_key` are 2.1).
/// A null slot means the function is absent.
///
/// All text is UTF-8 as a pointer and a length.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AmmApi {
    pub size: u32,
    pub abi_major: u32,
    pub abi_minor: u32,
    /// Game thread, at mod load. Registers a mod from its descriptor HCL text.
    /// `config_dir` is the mod's own AMML config folder as an absolute UTF-8 path with no `..` part; its
    /// `config.hcl` (the descriptor's `config_path`) is read and written there. Any other folder is
    /// `InvalidArgument`. `descriptor_hcl` and `config_dir` are valid only for the call. `callbacks` is copied
    /// during the call, up to the `size` it declares; a `size` below the 2.0 `AmmCallbacks` is `InvalidArgument`.
    /// `out_handle` is written on success.
    pub register: Option<
        unsafe extern "C" fn(
            descriptor_hcl: *const u8,
            descriptor_len: usize,
            config_dir: *const u8,
            config_dir_len: usize,
            callbacks: *const AmmCallbacks,
            out_handle: *mut AmmHandle,
        ) -> AmmStatus,
    >,
    /// Game thread, at mod unload. Releases everything the mod holds. It does not return while a `frame` or
    /// `textures_lost` callback of this mod runs on the render thread. On `Ok` no callback of this mod starts
    /// after it returns, so the mod may free its callback state at once. Any other status means the
    /// registration may still be live (for example `NotPermitted` when it is called from inside one of this
    /// mod's callbacks), so the mod must keep its callback state alive. A handle that names no live
    /// registration is `UnknownHandle`.
    pub unregister: Option<unsafe extern "C" fn(handle: AmmHandle) -> AmmStatus>,
    /// Game thread. The checked `config.hcl` text of the mod. The text at `out_ptr` is owned by the manager and
    /// is valid until the next `config_document` call for this handle or a `SettingsChanged` event.
    pub config_document:
        Option<unsafe extern "C" fn(handle: AmmHandle, out_ptr: *mut *const u8, out_len: *mut usize) -> AmmStatus>,
    /// Game thread. Asks for a capability lease. `capability` is an `AmmCapability` value; any other value is
    /// answered as denied with no holder.
    pub acquire: Option<unsafe extern "C" fn(handle: AmmHandle, capability: u8) -> AmmAcquireResult>,
    /// Game thread. Gives a lease back. `capability` is an `AmmCapability` value; any other value is ignored.
    pub release: Option<unsafe extern "C" fn(handle: AmmHandle, capability: u8)>,
    /// Any thread. The id of the mod that holds the lease now, copied as UTF-8 into `out_buf`, a buffer the
    /// caller owns with room for `out_cap` bytes. Returns the length of the id; 0 means no mod holds it, or
    /// `capability` is not an `AmmCapability` value. A return value greater than `out_cap` means the buffer was
    /// too small: nothing was written, and the caller may call again with a buffer of that length. `out_buf`
    /// may be null when `out_cap` is 0. The manager keeps no pointer to the buffer after the call.
    pub holder: Option<unsafe extern "C" fn(capability: u8, out_buf: *mut u8, out_cap: usize) -> u32>,
    /// Game thread. Drains the mod event queue into `out_buf`, a buffer the caller owns with room for `capacity`
    /// events, and returns the number written. The manager keeps no pointer to the buffer after the call.
    pub poll_events: Option<unsafe extern "C" fn(handle: AmmHandle, out_buf: *mut AmmEvent, capacity: usize) -> usize>,
    /// Game thread. Key state, answered only for the `Input` holder.
    pub key_down: Option<unsafe extern "C" fn(handle: AmmHandle, vk: u32) -> bool>,
    /// Game thread. A mod changes one of its own saved values with one HCL expression.
    /// `path` and `value_hcl` are valid only for the call.
    pub set_setting: Option<
        unsafe extern "C" fn(
            handle: AmmHandle,
            path: *const u8,
            path_len: usize,
            value_hcl: *const u8,
            value_len: usize,
        ) -> AmmStatus,
    >,
    /// Game thread. Turns the cursor mode on (1) or off (0). Any other value is ignored.
    pub set_cursor_mode: Option<unsafe extern "C" fn(handle: AmmHandle, on: u8)>,
    /// Any thread. One Curia log event from the mod. Never blocks. `level` is an `AmmLogLevel` value; any other
    /// value drops the event. `target`, `message`, `fields_json` and `file`
    /// are valid only for the call; the manager copies what it keeps.
    pub log: Option<
        unsafe extern "C" fn(
            handle: AmmHandle,
            level: u8,
            target: *const u8,
            target_len: usize,
            message: *const u8,
            message_len: usize,
            fields_json: *const u8,
            fields_len: usize,
            file: *const u8,
            file_len: usize,
            line: u32,
        ),
    >,
    /// Game thread. Asks for a game-only screenshot of the next Present, taken after the game has drawn
    /// and before any mod layer, toast or the panel, written as an opaque 8-bit RGB PNG to `path`. `path` is an
    /// absolute UTF-8 file path ending in `.png` (any case) with no `..` part, not a network, device or verbatim
    /// path, whose parent folder exists; any other path is `InvalidArgument`. The file is written atomically and
    /// replaces an existing file. The mod must hold an active `Hud` or `Overlay` lease, else `NotPermitted`. One
    /// capture of a mod is in flight at a time, from the request to its `ScreenshotWritten` event, and the
    /// manager has a fixed number of capture slots: past either, `Busy`. On `Ok`, `out_sequence` gets the capture
    /// sequence, never 0, that the `ScreenshotTaken` and `ScreenshotWritten` events carry in `code`. A null
    /// `path` or `out_sequence` is `NullPointer`, checked before the path; a path longer than 4096 bytes is
    /// `InvalidArgument`. `path` is valid only for the call. `request_screenshot_layers` with `layers` 0 does the
    /// same.
    pub request_screenshot: Option<
        unsafe extern "C" fn(handle: AmmHandle, path: *const u8, path_len: usize, out_sequence: *mut u32) -> AmmStatus,
    >,
    /// Game thread. 1 when a mod other than the caller holds `capability` now, else 0. A bad handle or
    /// a value that is not an `AmmCapability` gives 0. Called from a frame callback (render thread) it gives 0,
    /// because the host is busy composing the frame.
    pub held_by_other: Option<unsafe extern "C" fn(handle: AmmHandle, capability: u8) -> u8>,
    /// Game thread. The texture budget the manager gave the mod: at most `out_bytes` bytes of textures
    /// in at most `out_count` textures. Both out pointers are written on `Ok`; a null out pointer is
    /// `NullPointer`, and nothing is written.
    pub texture_budget:
        Option<unsafe extern "C" fn(handle: AmmHandle, out_bytes: *mut u64, out_count: *mut u32) -> AmmStatus>,
    /// Game thread. `request_screenshot` with a choice of capture point. `layers` is an
    /// `AmmScreenshotLayers` value: 0 (`GameOnly`) is taken after the game has drawn and before any mod layer, as
    /// `request_screenshot` is; 1 (`WithModLayers`) is taken after the last mod layer and before the manager's
    /// panel and toasts, so the picture shows the layers of all mods, as the player sees them, not only the
    /// caller's. Any other value is `InvalidArgument`, checked after the null pointers. The path, the lease (an
    /// active `Hud` or `Overlay` lease of the caller, for either value), one capture of a mod in flight at a time,
    /// the manager's capture slots, which both values share, the statuses and the `ScreenshotTaken` and
    /// `ScreenshotWritten` events are as for `request_screenshot`. `path` is valid only for the call.
    pub request_screenshot_layers: Option<
        unsafe extern "C" fn(
            handle: AmmHandle,
            path: *const u8,
            path_len: usize,
            layers: u8,
            out_sequence: *mut u32,
        ) -> AmmStatus,
    >,
    /// Game thread. Places a button for the mod in the corner row of the start and pause screens, left
    /// of the Mods button: a tile with `mark` (1 to 3 characters) and `label` (1 to 24 characters). `screens` is a
    /// bit set: 1 the start screen, 2 the pause screen; other bits are dropped, and no screen bit is
    /// `InvalidArgument`, as is a label or mark of the wrong length. On `Ok`, `out_id` gets the entry id, counted
    /// from 1 within the mod; a click on the entry sends the mod `MenuEntryClicked` with that id in `code`. A mod
    /// holds at most 4 entries and the manager 8, past either `QueueFull`. The entries go when the mod
    /// unregisters. A null text or `out_id` is `NullPointer`. `label` and `mark` are valid only for the call.
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
    /// Game thread. The language code the person picked in the manager, as UTF-8: empty for English,
    /// `en_XA` for the pseudo-locale, or a locale such as `ru` or `pt_BR`. It is copied into `out_buf`, a buffer
    /// the caller owns with room for `out_cap` bytes, and the call returns its length. A return value greater than
    /// `out_cap` means the buffer was too small: nothing was written, and the caller may call again with a buffer
    /// of that length. `out_buf` may be null when `out_cap` is 0. A handle that names no live registration gives
    /// 0. The first call marks the mod as a reader: only a reader gets `LocaleChanged`, so a mod built for 1.4 or
    /// older never sees that event kind. The manager keeps no pointer to the buffer after the call.
    pub locale: Option<unsafe extern "C" fn(handle: AmmHandle, out_buf: *mut u8, out_cap: usize) -> u32>,
    /// Game thread. Sets the mod's icon, shown in the manager panel, from the PNG file bytes at `png`.
    /// The file is at most 262144 bytes; its first frame is used, and its width and height are each 1 to 256
    /// pixels. A zero `png_len` clears the icon, and `png` may then be null. A null `png` with a `png_len` above 0
    /// is `NullPointer`. A file that is too large, is not a PNG, is outside the size range or is corrupt is
    /// `InvalidArgument`, and the icon the mod had stays. A handle that names no live registration is
    /// `UnknownHandle`. The bytes are checked before the handle, so bad bytes on an unknown handle are
    /// `InvalidArgument`. `png` is valid only for the call; the manager keeps a decoded copy. It may be called at any time after
    /// `register`. A new call replaces the current icon, and a `png_len` of 0 clears it, so the letter tile shows
    /// again. A decoder panic returns `Panic` and leaves the current icon as it was. The icon goes away when the
    /// mod unregisters.
    pub set_icon: Option<unsafe extern "C" fn(handle: AmmHandle, png: *const u8, png_len: usize) -> AmmStatus>,
    /// Game thread. Whether the mod may draw the live game picture, and its size. `Ok` while the mod
    /// holds `Overlay`, with the back buffer size of the last Present in physical pixels written to `out_width`
    /// and `out_height` (both 0 before the first Present). `NotPermitted` while it does not hold `Overlay`, and
    /// nothing is written. A null out pointer is `NullPointer`; a handle that names no live registration is
    /// `UnknownHandle`; a call from inside a frame callback is `Busy`. The mod draws the picture as the egui user
    /// texture `GamePicture::USER_ID` and uploads nothing; the manager drops the draw from a layer of a mod that
    /// does not hold `Overlay`.
    pub game_picture:
        Option<unsafe extern "C" fn(handle: AmmHandle, out_width: *mut u32, out_height: *mut u32) -> AmmStatus>,
    /// Game thread. Sets where the mod draws the game picture, `[min_x, min_y, max_x, max_y]` in
    /// back-buffer pixels, or clears it with a null `rect`. While the mod holds `Overlay`, a pointer inside it
    /// reaches the game at the matching place of the full window: its button is not kept from the game, and the
    /// game's own mouse position is moved there, so a game screen shown in the picture can be used. A new call
    /// replaces the rect. A rect with a NaN or no area is `InvalidArgument`; a handle that names no live
    /// registration is `UnknownHandle`; a call from inside a frame callback is `Busy`. The manager reads `rect`
    /// only during the call. The rect goes away when the mod unregisters.
    pub set_game_viewport: Option<unsafe extern "C" fn(handle: AmmHandle, rect: *const [f32; 4]) -> AmmStatus>,
    /// Game thread. Makes the game draw at `size`, in physical pixels, whatever the size of its window,
    /// or ends the size the mod forces with a null `size`. The picture is stretched into the window; the field of
    /// view does not change. The mod must hold `RenderSize`, else `NotPermitted`. Each side is 1 to 16384, else
    /// `InvalidArgument`. A handle that names no live registration is `UnknownHandle`; a call from inside a frame
    /// callback is `NotPermitted`. The game resizes within a few Presents: `render_size` reports when the back buffer has
    /// the size. The size ends, and the game gets its window size back, on a null `size`, when the mod no longer
    /// holds `RenderSize` (a release, a disable or an unregister), and when the manager shuts down. While the
    /// window is minimised the size change waits until the window has a size again. A resize of the window while
    /// a size is forced keeps the forced size. The manager reads `size` only during the call.
    pub set_render_size: Option<unsafe extern "C" fn(handle: AmmHandle, size: *const AmmRenderSize) -> AmmStatus>,
    /// Game thread. Writes to `out_state` the size the mod forces (0 x 0 for none) and the back buffer
    /// size and DXGI format of the last Present. `NotPermitted` while the mod does not hold `RenderSize`, and
    /// nothing is written. A null `out_state` is `NullPointer`; a handle that names no live registration is
    /// `UnknownHandle`; a call from inside a frame callback is `NotPermitted`. The mod sets `out_state.size` to the
    /// size of its struct; the manager writes only that many bytes and sets `size` to the bytes it wrote. A `size`
    /// below the 2.0 struct is `InvalidArgument`, and nothing is written.
    pub render_size: Option<unsafe extern "C" fn(handle: AmmHandle, out_state: *mut AmmRenderSizeState) -> AmmStatus>,
    /// Game thread. Asks the capture stream for one frame, tagged with `request.tag`: the next Present
    /// copies the back buffer after the game has drawn and before any mod layer, toast or the panel, and
    /// `capture_stream_take` gives the frames back in request order. The stream has 4 slots, shared by the frames
    /// asked for, in flight and not taken yet, apart from the screenshot slots. `Busy` when every slot is in use,
    /// or while the stream holds a frame of another mod: take a frame, then ask again. The mod must hold an active
    /// `Hud` or `Overlay` lease, else `NotPermitted`. A handle that names no live registration is `UnknownHandle`;
    /// a call from inside a frame callback is `NotPermitted`. `request.size` is the size of the mod's struct; the
    /// manager reads only that many bytes. A null `request` is `NullPointer`; a `size` below the 2.0 struct is
    /// `InvalidArgument`. A `request.present_seq` of 0 takes the frame from the next Present; any other number takes
    /// it from that Present only (see `present_count`): the frame stays pending until that Present, and fails
    /// `Missed` when that Present passed before the manager saw the request.
    pub capture_stream_request:
        Option<unsafe extern "C" fn(handle: AmmHandle, request: *const AmmCaptureStreamRequest) -> AmmStatus>,
    /// Game thread. Writes the mod's oldest stream frame to `out_frame` once it is done, and frees its
    /// slot: a frame with its pixels (lent until the next `capture_stream_take` that returns a frame, `capture_stream_close`, or
    /// until the mod unregisters), or a failed frame with its fault. A frame in flight when the back buffer changes
    /// size fails `Resized`; the frames asked for after the change come at the new size. `Busy`, with nothing written, while the oldest frame is
    /// still in flight or the mod has no frame. A null `out_frame` is `NullPointer`; a handle that names no live
    /// registration is `UnknownHandle`; a call from inside a frame callback is `NotPermitted`. The mod sets
    /// `out_frame.size` to the size of its struct; the manager writes only that many bytes and sets `size` to the
    /// bytes it wrote. A `size` below the 2.0 struct is `InvalidArgument`, checked before the frame is taken, so the
    /// frame stays in the stream.
    pub capture_stream_take:
        Option<unsafe extern "C" fn(handle: AmmHandle, out_frame: *mut AmmCaptureStreamFrame) -> AmmStatus>,
    /// Game thread. Writes to `out_free` how many frames the mod may ask the capture stream for now: the
    /// free slots, or 0 while the stream holds a frame of another mod. A null `out_free` is `NullPointer`; a handle
    /// that names no live registration is `UnknownHandle`; a call from inside a frame callback is `NotPermitted`.
    pub capture_stream_free: Option<unsafe extern "C" fn(handle: AmmHandle, out_free: *mut u32) -> AmmStatus>,
    /// Game thread. Ends the mod's capture stream: its frames are dropped, those in flight with them, and
    /// the lent pixels go. Once no frame is in the stream the manager frees every stream buffer, and the overlay drops
    /// its stream staging textures at the next Present. A later `capture_stream_request` starts a new stream.
    /// Unregister does the same. A handle that names no live registration is `UnknownHandle`; a call from inside a
    /// frame callback is `NotPermitted`.
    pub capture_stream_close: Option<unsafe extern "C" fn(handle: AmmHandle) -> AmmStatus>,
    /// Game thread. Writes to `out_count` the overlay's Present count: the number of the newest Present the
    /// overlay started, 0 before the first. A `capture_stream_request` with `present_seq` set to a number above
    /// this count takes its frame from that Present. A null `out_count` is `NullPointer`; a handle that names no
    /// live registration is `UnknownHandle`.
    pub present_count: Option<unsafe extern "C" fn(handle: AmmHandle, out_count: *mut u64) -> AmmStatus>,
    /// Game thread. As `capture_stream_take`, but the manager lends the frame's pixel buffer to the mod until
    /// `capture_stream_give_back` with the `lend_id` the frame carries, however many takes and lends come between,
    /// and the mod may read the pixels from any thread; the manager never writes a lent buffer. At most 8 buffers
    /// are lent at once: a lend with 8 out is `Busy`, with nothing written and the frame still in the stream, as is
    /// a lend while the oldest frame is in flight or the mod has none. A failed frame carries its fault and no
    /// lease. `capture_stream_close` keeps the lent buffers until they are given back; unregister frees them.
    /// Errors, and the `out_frame.size` rules, as `capture_stream_take`.
    pub capture_stream_lend:
        Option<unsafe extern "C" fn(handle: AmmHandle, out_frame: *mut AmmCaptureStreamFrame) -> AmmStatus>,
    /// Game thread. Ends the lend `lend_id` of this mod: the manager reuses the buffer for a later frame, and the
    /// mod must not read its pixels after the call. A `lend_id` the mod does not hold (unknown, already given back,
    /// or from another mod) is `InvalidArgument`; a give-back after `capture_stream_close` is `Ok`. A handle that
    /// names no live registration is `UnknownHandle`.
    pub capture_stream_give_back: Option<unsafe extern "C" fn(handle: AmmHandle, lend_id: u64) -> AmmStatus>,
    /// Since 2.1. Game thread. Sets the keys the mod wants to see, whatever else the key does, replacing the list of
    /// an earlier call; a `count` of 0 clears it, and `vks` may then be null. `vks` holds `count` Windows
    /// virtual-key codes, each 0 to 255, and at most 32 keys; a key above 255 or a longer list is `InvalidArgument`,
    /// and the list the mod had stays. The mod needs an `Input` claim in its descriptor, else `NotPermitted`. A null
    /// `vks` with a `count` above 0 is `NullPointer`, checked before the claim; a handle that names no live
    /// registration is `UnknownHandle`. `vks` is valid only for the call. The list goes when the mod unregisters or
    /// is disabled.
    pub watch_keys: Option<unsafe extern "C" fn(handle: AmmHandle, vks: *const u32, count: usize) -> AmmStatus>,
    /// Since 2.1. Game thread. Holds a key of the game down (`down` 1) or lets it go (`down` 0) with a fake press
    /// that the game sees as its own keyboard. Any other `down` value is `InvalidArgument`, as is a `vk` above 255.
    /// The mod needs an `Input` claim in its descriptor, else `NotPermitted`; so is any call while the manager panel
    /// is open, and a `down` of 1 while another mod holds an active `Input` lease. A `down` of 1 for a key the mod
    /// already holds, or a `down` of 0 for a key it does not hold, is `Ok` and sends nothing. The manager lets go of
    /// the keys a mod holds when it unregisters or is disabled, when the panel opens, when the game window loses
    /// focus and when the world is left; the mod, if still registered, then gets `FakeKeysReleased`. A handle that
    /// names no live registration is `UnknownHandle`.
    pub send_key: Option<unsafe extern "C" fn(handle: AmmHandle, vk: u32, down: u8) -> AmmStatus>,
}

impl AbiSized for AmmApi {
    const BASE_SIZE: usize = size_of::<Self>();

    fn blank() -> Self {
        Self {
            size: size_of::<Self>() as u32,
            abi_major: 0,
            abi_minor: 0,
            register: None,
            unregister: None,
            config_document: None,
            acquire: None,
            release: None,
            holder: None,
            poll_events: None,
            key_down: None,
            set_setting: None,
            set_cursor_mode: None,
            log: None,
            request_screenshot: None,
            held_by_other: None,
            texture_budget: None,
            request_screenshot_layers: None,
            register_menu_entry: None,
            locale: None,
            set_icon: None,
            game_picture: None,
            set_game_viewport: None,
            set_render_size: None,
            render_size: None,
            capture_stream_request: None,
            capture_stream_take: None,
            capture_stream_free: None,
            capture_stream_close: None,
            present_count: None,
            capture_stream_lend: None,
            capture_stream_give_back: None,
            watch_keys: None,
            send_key: None,
        }
    }
}
