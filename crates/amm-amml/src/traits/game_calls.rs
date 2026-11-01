use std::time::Duration;

use amm_abi::types::AmmStatus;
use amm_core::structs::input::FakeKeyOp;
use amm_overlay::{PointerButton, ShutdownOutcome, UninstallOutcome};
use amml_msvc::MsvcString;
use amml_sdk::SdkError;
use amml_sdk::amml_abi::AmmlLogLevel;

use crate::paths::ManagerPaths;
use crate::types::{HookGroup, RenderRestore};

/// Everything the manager's services call outside their own state: the manager host, the AMML loader, the
/// overlay, the window and the game. `LiveCalls` makes the real calls; tests record them.
///
/// A game call takes the object it is made on as an address; the services never pass 0.
pub trait GameCalls: Sync {
    /// A clock in milliseconds, the one the host's tick takes.
    fn now_ms(&self) -> u64;

    /// `amm_host_create`. A host that already runs is kept, with `Ok`.
    fn create_host(&self, paths: &ManagerPaths) -> AmmStatus;

    /// `amm_host_set_console`: the host's Warn and Error lines go to the AMML log while `on`.
    fn set_console(&self, on: bool);

    /// `amm_host_destroy`.
    fn destroy_host(&self);

    /// `amm_host_tick`.
    fn host_tick(&self, now_ms: u64);

    /// `amm_host_world`.
    fn host_world(&self, loaded: bool);

    /// `amm_host_key`; true when the game must not see the key. `mods` is 1 Ctrl, 2 Shift, 4 Alt.
    fn host_key(&self, vk: u32, down: bool, repeat: bool, mods: u8, now_ms: u64) -> bool;

    /// `amm_host_take_fake_keys` until the host has none left: appends the queued presses to `out`, oldest first.
    fn take_fake_keys(&self, out: &mut Vec<FakeKeyOp>);

    /// `amm_host_release_fake_keys`: the host queues the release of every fake key a mod holds.
    fn release_fake_keys(&self);

    /// The game's own `onKeyDown` (`down`) or `onKeyUp` on the HIDController `controller`, past the manager's key
    /// detour. `origin` is the `InputOrigin` of a key down and is ignored for a key up.
    fn game_key(&self, controller: usize, vk: u32, down: bool, origin: i32);

    /// `amm_host_pointer` with a raw mouse movement; true when the game must not see it.
    fn host_pointer_delta(&self, dx: f32, dy: f32) -> bool;

    /// `amm_host_blocks_player_turn`.
    fn blocks_player_turn(&self) -> bool;

    /// The modifier mask of the keys held now: 1 Ctrl, 2 Shift, 4 Alt.
    fn modifiers(&self) -> u8;

    /// `WndProcRouter::take_focus_lost`.
    fn take_focus_lost(&self) -> bool;

    /// `WndProcRouter::consumed`.
    fn button_consumed(&self, button: PointerButton) -> bool;

    /// The game's mouse position, in client pixels, for the real cursor while it is inside a game viewport; `None`
    /// outside one.
    fn game_pointer(&self) -> Option<(i16, i16)>;

    /// The overlay's game window is the foreground window. Asked before the overlay shuts down, which forgets the
    /// window.
    fn game_window_foreground(&self) -> bool;

    /// Installs every game hook after reading the `GameSlot` entries. A missing required entry, or a hook the
    /// loader refuses, fails with nothing left installed; a missing optional entry turns off its own feature with
    /// one log line.
    fn install_hooks(&self) -> Result<(), SdkError>;

    /// Removes the hooks of `group`; none installed is fine.
    fn remove_hooks(&self, group: HookGroup);

    /// Hooks the swap chain. A failure leaves the game without layers and is logged; it does not fail enable.
    fn install_overlay(&self);

    /// `WndProcRouter::uninstall`.
    fn uninstall_wndproc(&self) -> UninstallOutcome;

    /// `Overlay::shutdown`; `Drained` when no overlay is installed.
    fn shutdown_overlay(&self, timeout: Duration) -> ShutdownOutcome;

    /// `amm_host_render_size_shutdown` with the game window's client size: the host forces no render size from
    /// now on. Gives the size message the game needs when a forced size was in place; posts nothing. Asked after
    /// the window procedure is gone and before the overlay shuts down.
    fn close_render_size(&self) -> Option<RenderRestore>;

    /// Posts `restore` to the game window as `WM_SIZE`. Asked after the overlay shut down, so no frame can post a
    /// size message after it.
    fn post_render_restore(&self, restore: RenderRestore);

    /// `ClientInstance::getLocalPlayer`; 0 for none.
    fn local_player(&self, client: usize) -> usize;

    /// `ClientInstance::isWorldActive`.
    fn world_active(&self, client: usize) -> bool;

    /// `ClientInstance::isShowingMenu`.
    fn showing_menu(&self, client: usize) -> bool;

    /// `IMinecraftGame::releaseMouse`.
    fn release_mouse(&self, game: usize);

    /// `IMinecraftGame::grabMouse`.
    fn grab_mouse(&self, game: usize);

    /// `ClientInstance::getTopScreenName`, a string the game allocated; `None` while the menu screen is off.
    fn top_screen_name(&self, client: usize) -> Option<MsvcString>;

    /// Frees a string `top_screen_name` returned, through the game heap.
    fn release_string(&self, text: MsvcString);

    /// `ClientInstance::getOptions`, the `IOptionRegistry` of the client; `None` while hide HUD is off.
    fn options(&self, client: usize) -> Option<usize>;

    /// `IOptionRegistry::getHideHud`.
    fn hide_hud(&self, options: usize) -> bool;

    /// `IOptionRegistry::setHideGUI`.
    fn set_hide_gui(&self, options: usize, hide: bool);

    /// One line in the manager's AMML log.
    fn log(&self, level: AmmlLogLevel, message: &str);
}
