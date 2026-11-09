#![allow(unsafe_code)]

use std::ffi::c_void;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use amm_abi::convert::pointer_op::PointerOpCodec;
use amm_abi::types::AmmStatus;
use amm_core::structs::input::{FakeKeyOp, PointerOp};
use amm_ffi::host::{
    amm_host_blocks_gamepad, amm_host_blocks_player_turn, amm_host_create, amm_host_destroy, amm_host_game_point,
    amm_host_key, amm_host_pointer, amm_host_release_fake_keys, amm_host_render_size_shutdown, amm_host_set_console,
    amm_host_take_fake_keys, amm_host_take_fake_pointer, amm_host_take_focus_loss_owed, amm_host_tick, amm_host_world,
};
use amm_ffi::types::{AmmHostFakeKey, AmmHostFakePointer, AmmHostPointerKind};
use amm_overlay::{Overlay, PointerButton, ShutdownOutcome, SizeMessage, UninstallOutcome, WndProcRouter};
use amml_msvc::{MemberCall, MsvcString};
use amml_sdk::amml_abi::AmmlLogLevel;
use amml_sdk::{Api, SdkError};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, GetCursorPos, GetForegroundWindow};

use super::{
    AmmlHookHost, AmmlHookInstaller, ConsoleMirror, GameHookSet, GamePointer, GameSlots, LoaderHeap, ManagerInputSink,
    ManagerOverlayHost,
};
use crate::game_hooks::{apply_turn_delta, feed, key_down, key_up};
use crate::manager_mod::ManagerMod;
use crate::paths::ManagerPaths;
use crate::traits::GameCalls;
use crate::types::{GameSlot, HookGroup, RenderRestore};

/// The real calls: the `amm_host_*` API, the AMML loader, the overlay, the window, and the game through the
/// vtable slots `GameSlots` read at enable.
pub struct LiveCalls {
    api: Api,
    host: &'static ManagerOverlayHost,
    sink: &'static ManagerInputSink,
    hooks: GameHookSet,
    slots: GameSlots,
    overlay: Mutex<Option<Overlay<AmmlHookInstaller>>>,
}

impl LiveCalls {
    const KEY_HELD: i16 = i16::MIN;
    const MOD_CTRL: u8 = 1;
    const MOD_SHIFT: u8 = 2;
    const MOD_ALT: u8 = 4;
    const FAKE_KEY_BATCH: usize = 64;
    const FAKE_POINTER_BATCH: usize = 64;
    const HUD_SLOTS: [GameSlot; 3] = [GameSlot::Options, GameSlot::SetHideGui, GameSlot::GetHideHud];

    pub fn new(api: Api, host: &'static ManagerOverlayHost, sink: &'static ManagerInputSink) -> Self {
        Self { api, host, sink, hooks: GameHookSet::new(), slots: GameSlots::new(), overlay: Mutex::new(None) }
    }

    fn lock_overlay(&self) -> MutexGuard<'_, Option<Overlay<AmmlHookInstaller>>> {
        self.overlay.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn loader(&self) -> AmmlHookHost<'_> {
        AmmlHookHost::new(&self.api, ManagerMod::MODULE)
    }

    // the function in `slot` of the vtable at the start of `object`, when the slot resolved
    fn virtual_fn(&self, object: usize, slot: GameSlot) -> Option<*const c_void> {
        let index = self.slots.get(slot)?;
        // SAFETY: `object` is a live game object whose first qword is the vtable of the class the slot entry
        // names (ClientInstance, MinecraftGame or OptionRegistry), and the recipe range checks keep the slot
        // inside that vtable
        let function = unsafe { *(*(object as *const *const usize)).add(index) };
        Some(function as *const c_void)
    }

    fn key_held(key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
        // SAFETY: no input but the key code
        let state = unsafe { GetAsyncKeyState(i32::from(key.0)) };
        state & Self::KEY_HELD != 0
    }
}

impl GameCalls for LiveCalls {
    fn now_ms(&self) -> u64 {
        // SAFETY: no input
        unsafe { GetTickCount64() }
    }

    fn create_host(&self, paths: &ManagerPaths) -> AmmStatus {
        let (Some(config), Some(game)) = (paths.config_dir.to_str(), paths.game_dir.to_str()) else {
            return AmmStatus::InvalidArgument;
        };
        let version = paths.version.as_str();
        // SAFETY: the three texts live for the call
        unsafe {
            amm_host_create(config.as_ptr(), config.len(), game.as_ptr(), game.len(), version.as_ptr(), version.len())
        }
    }

    fn set_console(&self, on: bool) {
        let mirror: unsafe extern "C" fn(u8, *const u8, usize) = ConsoleMirror::write;
        amm_host_set_console(on.then_some(mirror));
    }

    fn destroy_host(&self) {
        amm_host_destroy();
    }

    fn host_tick(&self, now_ms: u64) {
        amm_host_tick(now_ms);
    }

    fn host_world(&self, loaded: bool) {
        amm_host_world(loaded);
    }

    fn host_key(&self, vk: u32, down: bool, repeat: bool, mods: u8, now_ms: u64) -> bool {
        amm_host_key(vk, down, repeat, mods, now_ms).consume
    }

    fn take_fake_keys(&self, out: &mut Vec<FakeKeyOp>) {
        let mut batch = [AmmHostFakeKey { vk: 0, down: 0 }; Self::FAKE_KEY_BATCH];
        loop {
            // SAFETY: `batch` is a writable local with room for `FAKE_KEY_BATCH` entries
            let taken = unsafe { amm_host_take_fake_keys(batch.as_mut_ptr(), batch.len()) };
            out.extend(batch.iter().take(taken).map(|key| FakeKeyOp { vk: key.vk, down: key.down != 0 }));
            if taken < batch.len() {
                return;
            }
        }
    }

    fn release_fake_keys(&self) {
        amm_host_release_fake_keys();
    }

    fn game_key(&self, controller: usize, vk: u32, down: bool, origin: i32) {
        // the Windows VK code goes to the game unchanged
        let key = vk as i32;
        if down {
            let original: extern "C" fn(usize, i32, i32) = key_down::GUARD.original_fn();
            original(controller, key, origin);
        } else {
            let original: extern "C" fn(usize, i32) = key_up::GUARD.original_fn();
            original(controller, key);
        }
    }

    fn take_fake_pointer(&self, out: &mut Vec<PointerOp>) {
        let mut batch = [AmmHostFakePointer { op: 0, value: 0 }; Self::FAKE_POINTER_BATCH];
        loop {
            // SAFETY: `batch` is a writable local with room for `FAKE_POINTER_BATCH` entries
            let taken = unsafe { amm_host_take_fake_pointer(batch.as_mut_ptr(), batch.len()) };
            out.extend(batch.iter().take(taken).filter_map(|entry| PointerOpCodec::decode(entry.op, entry.value).ok()));
            if taken < batch.len() {
                return;
            }
        }
    }

    fn game_mouse(&self, device: usize, action: u8, data: i8, x: i16, y: i16) {
        let original: extern "C" fn(usize, u8, i8, i16, i16, i16, i16, u8) = feed::GUARD.original_fn();
        original(device, action, data, x, y, 0, 0, 0);
    }

    fn game_turn(&self, player: usize, pitch: f32, yaw: f32) {
        // the slot is null while the turn hook is not installed; the trampoline then is not there to call
        if apply_turn_delta::GUARD.original().load(Ordering::Acquire).is_null() {
            return;
        }
        // `Vec2 { x: pitch, y: yaw }`, read by the game during the call
        let delta = [pitch, yaw];
        let original: extern "C" fn(usize, usize) = apply_turn_delta::GUARD.original_fn();
        original(player, delta.as_ptr() as usize);
    }

    fn host_pointer_delta(&self, dx: f32, dy: f32) -> bool {
        amm_host_pointer(AmmHostPointerKind::Delta as u8, dx, dy, 0, false, 0.0).consume
    }

    fn blocks_player_turn(&self) -> bool {
        amm_host_blocks_player_turn()
    }

    fn blocks_gamepad(&self) -> bool {
        amm_host_blocks_gamepad()
    }

    fn modifiers(&self) -> u8 {
        let mut mods = 0;
        if Self::key_held(VK_CONTROL) {
            mods |= Self::MOD_CTRL;
        }
        if Self::key_held(VK_SHIFT) {
            mods |= Self::MOD_SHIFT;
        }
        if Self::key_held(VK_MENU) {
            mods |= Self::MOD_ALT;
        }
        mods
    }

    fn take_focus_lost(&self) -> bool {
        WndProcRouter::take_focus_lost()
    }

    fn button_consumed(&self, button: PointerButton) -> bool {
        WndProcRouter::consumed(button)
    }

    fn game_pointer(&self) -> Option<(i16, i16)> {
        let window = Overlay::<AmmlHookInstaller>::game_window()?;
        let mut client = RECT::default();
        let mut cursor = POINT::default();
        // SAFETY: both out pointers are writable locals; a window that is gone fails the call
        unsafe { GetClientRect(window, &mut client) }.ok()?;
        // SAFETY: as above
        unsafe { GetCursorPos(&mut cursor) }.ok()?;
        // SAFETY: as above
        if !unsafe { ScreenToClient(window, &mut cursor) }.as_bool() {
            return None;
        }
        let buffer = Overlay::<AmmlHookInstaller>::back_buffer_size();
        GamePointer::map((cursor.x, cursor.y), (client.right, client.bottom), buffer, |x, y, client_w, client_h| {
            let (mut game_x, mut game_y) = (0.0f32, 0.0f32);
            // SAFETY: both out pointers are writable locals
            unsafe { amm_host_game_point(x, y, client_w, client_h, &mut game_x, &mut game_y) }
                .then_some((game_x, game_y))
        })
    }

    fn game_window_foreground(&self) -> bool {
        // SAFETY: no input
        let foreground = unsafe { GetForegroundWindow() };
        Overlay::<AmmlHookInstaller>::game_window().is_some_and(|window| window == foreground)
    }

    fn take_focus_loss_owed(&self) -> bool {
        amm_host_take_focus_loss_owed()
    }

    fn replay_focus_loss(&self) {
        WndProcRouter::replay_focus_loss();
    }

    fn install_hooks(&self) -> Result<(), SdkError> {
        let loader = self.loader();
        self.slots.resolve(&loader)?;
        self.hooks.install(&loader, &ManagerMod::hooks())
    }

    fn remove_hooks(&self, group: HookGroup) {
        self.hooks.remove(&self.loader(), group);
    }

    fn install_overlay(&self) {
        match Overlay::install(AmmlHookInstaller::new(self.api), self.host, Some(self.sink)) {
            Ok(overlay) => *self.lock_overlay() = Some(overlay),
            Err(error) => self.log(AmmlLogLevel::WARN, &format!("the overlay is off: {error}")),
        }
    }

    fn uninstall_wndproc(&self) -> UninstallOutcome {
        WndProcRouter::uninstall()
    }

    fn shutdown_overlay(&self, timeout: Duration) -> ShutdownOutcome {
        let overlay = self.lock_overlay().take();
        overlay.map_or(ShutdownOutcome::Drained, |overlay| overlay.shutdown(timeout))
    }

    fn close_render_size(&self) -> Option<RenderRestore> {
        let window = Overlay::<AmmlHookInstaller>::game_window()?;
        let mut client = RECT::default();
        // SAFETY: the out pointer is a writable local; a window that is gone fails the call
        if unsafe { GetClientRect(window, &mut client) }.is_err() {
            return None;
        }
        let client_width = client.right.max(0) as u32;
        let client_height = client.bottom.max(0) as u32;
        let (mut width, mut height) = (0u32, 0u32);
        // SAFETY: both out pointers are writable locals
        let due = unsafe { amm_host_render_size_shutdown(client_width, client_height, &mut width, &mut height) };
        due.then_some(RenderRestore { window: window.0 as usize, width, height })
    }

    fn post_render_restore(&self, restore: RenderRestore) {
        SizeMessage::post(HWND(restore.window as *mut c_void), restore.width, restore.height, "shutdown");
    }

    fn local_player(&self, client: usize) -> usize {
        let Some(function) = self.virtual_fn(client, GameSlot::LocalPlayer) else {
            return 0;
        };
        // SAFETY: the slot holds `LocalPlayer* ClientInstance::getLocalPlayer() const`
        unsafe { std::mem::transmute::<*const c_void, extern "C" fn(usize) -> usize>(function)(client) }
    }

    fn world_active(&self, client: usize) -> bool {
        let Some(function) = self.virtual_fn(client, GameSlot::WorldActive) else {
            return false;
        };
        // SAFETY: the slot holds `bool ClientInstance::isWorldActive() const`, read as a byte
        unsafe { std::mem::transmute::<*const c_void, extern "C" fn(usize) -> u8>(function)(client) != 0 }
    }

    fn showing_menu(&self, client: usize) -> bool {
        let Some(function) = self.virtual_fn(client, GameSlot::ShowingMenu) else {
            return false;
        };
        // SAFETY: the slot holds `bool ClientInstance::isShowingMenu() const`, read as a byte
        unsafe { std::mem::transmute::<*const c_void, extern "C" fn(usize) -> u8>(function)(client) != 0 }
    }

    fn release_mouse(&self, game: usize) {
        if let Some(function) = self.virtual_fn(game, GameSlot::ReleaseMouse) {
            // SAFETY: `game` is the MinecraftGame the game passed to its own grabMouse, whose first vtable is
            // IMinecraftGame's, and the slot holds `void IMinecraftGame::releaseMouse()`
            unsafe { std::mem::transmute::<*const c_void, extern "C" fn(usize)>(function)(game) };
        }
    }

    fn grab_mouse(&self, game: usize) {
        if let Some(function) = self.virtual_fn(game, GameSlot::GrabMouse) {
            // SAFETY: as release_mouse; the slot holds `void IMinecraftGame::grabMouse()`
            unsafe { std::mem::transmute::<*const c_void, extern "C" fn(usize)>(function)(game) };
        }
    }

    fn top_screen_name(&self, client: usize) -> Option<MsvcString> {
        LoaderHeap::msvc()?;
        let function = self.virtual_fn(client, GameSlot::TopScreenName)?;
        // SAFETY: the slot holds `std::string ClientInstance::getTopScreenName() const`, a member function that
        // returns the string through a hidden pointer; MsvcString has the layout of the release std::string
        Some(unsafe { MemberCall::call_sret0::<MsvcString>(function, client as *mut c_void) })
    }

    fn release_string(&self, mut text: MsvcString) {
        if let Some(heap) = LoaderHeap::msvc() {
            // SAFETY: the string came from the game's std::string, which allocates through the game heap that
            // `LoaderHeap` reaches
            unsafe { text.release(&heap) };
        }
    }

    fn options(&self, client: usize) -> Option<usize> {
        if !self.slots.all(&Self::HUD_SLOTS) {
            return None;
        }
        let function = self.virtual_fn(client, GameSlot::Options)?;
        // SAFETY: the slot holds `IOptionRegistry& ClientInstance::getOptions()`, a reference returned as a pointer
        let options = unsafe { std::mem::transmute::<*const c_void, extern "C" fn(usize) -> usize>(function)(client) };
        (options != 0).then_some(options)
    }

    fn hide_hud(&self, options: usize) -> bool {
        let Some(function) = self.virtual_fn(options, GameSlot::GetHideHud) else {
            return false;
        };
        // SAFETY: the slot holds `bool IOptionRegistry::getHideHud() const`, read as a byte
        unsafe { std::mem::transmute::<*const c_void, extern "C" fn(usize) -> u8>(function)(options) != 0 }
    }

    fn set_hide_gui(&self, options: usize, hide: bool) {
        if let Some(function) = self.virtual_fn(options, GameSlot::SetHideGui) {
            // SAFETY: the slot holds `void IOptionRegistry::setHideGUI(bool)`
            unsafe {
                std::mem::transmute::<*const c_void, extern "C" fn(usize, u8)>(function)(options, u8::from(hide))
            };
        }
    }

    fn log(&self, level: AmmlLogLevel, message: &str) {
        self.api.log(ManagerMod::MODULE, level, message);
    }
}
