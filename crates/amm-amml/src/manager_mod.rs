use std::ffi::c_void;

use amml_sdk::{AmmlMod, Api, SdkError};
use windows::Win32::Foundation::HMODULE;

use crate::dup_guard::DupGuard;
use crate::errors::DupError;
use crate::game_hooks::{
    apply_turn_delta, client_update, feed, grab_mouse, key_down, key_up, level_exit, player_loaded,
};
use crate::paths::ManagerPaths;
use crate::services::{LiveCalls, ManagerInputSink, ManagerOverlayHost, ManagerRuntime, ManagerState};
use crate::types::{HookGroup, HookSpec};

/// The AMML entry point. Load checks that this copy is the module mods find and records the folders; it starts no
/// thread. Enable starts the host, then the hooks and the overlay. Disable takes them down in the order of
/// `ManagerLifecycle`.
pub struct ManagerMod;

impl ManagerMod {
    /// The module string the loader knows this mod by: `@<NAMESPACE>/<NAME>`.
    pub const MODULE: &'static str = "@amml/modmanager";
    pub const UPDATE: &'static str = "ClientInstance.update";
    pub const PLAYER_LOADED: &'static str = "ClientInstance.onPlayerLoaded";
    pub const LEVEL_EXIT: &'static str = "ClientExitLevel.hook";
    pub const MOUSE_FEED: &'static str = "MouseDevice.feed";
    pub const KEY_DOWN: &'static str = "HIDControllerGameCoreDesktop.onKeyDown";
    pub const KEY_UP: &'static str = "HIDControllerGameCoreDesktop.onKeyUp";
    pub const GRAB_MOUSE: &'static str = "MinecraftGame.grabMouse";
    pub const TURN_DELTA: &'static str = "LocalPlayer.applyTurnDelta";

    /// Every game hook, in install order; the tick goes in last.
    pub fn hooks() -> [HookSpec; 8] {
        [
            HookSpec {
                entry: Self::PLAYER_LOADED,
                detour: player_loaded::detour as *const c_void,
                original: player_loaded::GUARD.original(),
                group: HookGroup::World,
                required: false,
                feature: "the world join event",
            },
            HookSpec {
                entry: Self::LEVEL_EXIT,
                detour: level_exit::detour as *const c_void,
                original: level_exit::GUARD.original(),
                group: HookGroup::World,
                required: false,
                feature: "the world exit event",
            },
            HookSpec {
                entry: Self::MOUSE_FEED,
                detour: feed::detour as *const c_void,
                original: feed::GUARD.original(),
                group: HookGroup::Input,
                required: true,
                feature: "the manager",
            },
            HookSpec {
                entry: Self::KEY_DOWN,
                detour: key_down::detour as *const c_void,
                original: key_down::GUARD.original(),
                group: HookGroup::Input,
                required: true,
                feature: "the manager",
            },
            HookSpec {
                entry: Self::KEY_UP,
                detour: key_up::detour as *const c_void,
                original: key_up::GUARD.original(),
                group: HookGroup::Input,
                required: true,
                feature: "the manager",
            },
            HookSpec {
                entry: Self::GRAB_MOUSE,
                detour: grab_mouse::detour as *const c_void,
                original: grab_mouse::GUARD.original(),
                group: HookGroup::Grab,
                required: true,
                feature: "the manager",
            },
            HookSpec {
                entry: Self::TURN_DELTA,
                detour: apply_turn_delta::detour as *const c_void,
                original: apply_turn_delta::GUARD.original(),
                group: HookGroup::Turn,
                required: false,
                feature: "the look turn block",
            },
            HookSpec {
                entry: Self::UPDATE,
                detour: client_update::detour as *const c_void,
                original: client_update::GUARD.original(),
                group: HookGroup::Tick,
                required: true,
                feature: "the manager",
            },
        ]
    }

    /// Ok when `own` is the module mods find by name. Both `load` and `enable` ask: a C++ manager that loaded after
    /// our `load` is seen at `enable`, because LeviLamina loads every mod before it enables one.
    pub fn check_sole_copy(own: Result<HMODULE, DupError>) -> Result<(), SdkError> {
        own.and_then(DupGuard::check).map_err(|error| SdkError::Mod(error.to_string()))
    }

    /// Runs `start` only when `own` is the sole manager copy.
    pub fn enable_if_sole(
        own: Result<HMODULE, DupError>,
        start: impl FnOnce() -> Result<(), SdkError>,
    ) -> Result<(), SdkError> {
        Self::check_sole_copy(own)?;
        start()
    }

    fn runtime() -> Result<&'static ManagerRuntime, SdkError> {
        ManagerRuntime::get().ok_or_else(|| SdkError::Mod("load did not run".to_string()))
    }
}

impl AmmlMod for ManagerMod {
    const NAMESPACE: &'static str = "amml";
    const NAME: &'static str = "modmanager";

    fn load(api: &Api) -> Result<(), SdkError> {
        Self::check_sole_copy(DupGuard::own_module())?;
        let paths = ManagerPaths::from_api(api, &Self::id())?;
        let state = ManagerState::leaked();
        let host = ManagerOverlayHost::leaked(state);
        let sink = ManagerInputSink::leaked(state);
        let calls: &'static LiveCalls = Box::leak(Box::new(LiveCalls::new(*api, host, sink)));
        ManagerRuntime::install(ManagerRuntime::new(state, calls, paths));
        Ok(())
    }

    fn enable(_api: &Api) -> Result<(), SdkError> {
        Self::enable_if_sole(DupGuard::own_module(), || Self::runtime()?.enable())
    }

    fn disable(_api: &Api) {
        if let Some(runtime) = ManagerRuntime::get() {
            runtime.disable();
        }
    }
}
