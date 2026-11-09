use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use amm_abi::types::AmmStatus;
use amm_core::services::render::RenderSizeBook;
use amm_core::structs::render::RenderSize;
use arc_swap::ArcSwapOption;

use super::{HostRuntime, HostShared, LogBridge, ReentryGuard};
use crate::types::{AmmHostConsoleCallback, AmmHostFrame, AmmHostFrameInput};

/// The one runtime handle of the manager DLL: the host behind a lock, the state the render thread and `log`
/// read without the lock, and the console callback of the host mod.
pub struct HostCell {
    runtime: Mutex<Option<HostRuntime>>,
    shared: ArcSwapOption<HostShared>,
    console: Mutex<Option<AmmHostConsoleCallback>>,
}

impl HostCell {
    pub fn global() -> &'static HostCell {
        static CELL: OnceLock<HostCell> = OnceLock::new();
        CELL.get_or_init(|| Self {
            runtime: Mutex::new(None),
            shared: ArcSwapOption::empty(),
            console: Mutex::new(None),
        })
    }

    /// Starts the host. A host that already runs is kept.
    pub fn create(&self, config_dir: PathBuf, game_dir: PathBuf, version: String) -> Result<(), AmmStatus> {
        let _inside = ReentryGuard::enter().ok_or(AmmStatus::NotPermitted)?;
        let mut runtime = self.lock();
        if let Some(running) = runtime.as_mut() {
            running.render_size_open();
            curia::warn!("mod manager host created twice, the running host is kept");
            return Ok(());
        }
        let created = HostRuntime::new(config_dir, game_dir, version).map_err(|error| AmmStatus::from(&error))?;
        self.shared.store(Some(Arc::clone(created.shared())));
        *runtime = Some(created);
        Ok(())
    }

    /// Stops the host: no call reaches it after this begins, and pending writes are flushed within a bound.
    pub fn destroy(&self) {
        let Some(_inside) = ReentryGuard::enter() else {
            return;
        };
        let taken = self.lock().take();
        self.shared.store(None);
        if let Some(runtime) = taken {
            runtime.shutdown();
        }
    }

    /// Runs `f` on the host under the lock, then publishes the lock-free state. `ManagerMissing` without a host;
    /// `NotPermitted` when this thread already holds the lock, as a callback calling back into the table does.
    pub fn with<T>(&self, f: impl FnOnce(&mut HostRuntime) -> Result<T, AmmStatus>) -> Result<T, AmmStatus> {
        let _inside = ReentryGuard::enter().ok_or(AmmStatus::NotPermitted)?;
        let mut runtime = self.lock();
        let runtime = runtime.as_mut().ok_or(AmmStatus::ManagerMissing)?;
        let result = f(runtime);
        runtime.publish_shared();
        result
    }

    /// The lock-free host state, while a host runs.
    pub fn shared(&self) -> Option<Arc<HostShared>> {
        self.shared.load_full()
    }

    /// Composes one Present. With no layer, no toast, the panel closed, no menu button to show or take away, no
    /// capture in flight and no render size work it answers from the lock-free state.
    pub fn frame(&self, input: &AmmHostFrameInput) -> Result<AmmHostFrame, AmmStatus> {
        {
            let shared = self.shared.load();
            let shared = shared.as_ref().ok_or(AmmStatus::ManagerMissing)?;
            shared.set_back_buffer(RenderSize::new(input.width, input.height), input.format);
            let snapshot = shared.snapshot().load();
            let button = snapshot.menu_button && (input.menu_screen != 0 || shared.menu_button_placed());
            let idle = snapshot.layers.is_empty()
                && !snapshot.panel_open
                && !button
                && !shared.capturing()
                && !shared.render_size_busy();
            if idle && !shared.toast_visible(input.now_ms) {
                return Ok(AmmHostFrame {
                    freed_owners: shared.take_freed(),
                    cursor_free: snapshot.cursor_free,
                    hide_vanilla_hud: snapshot.hide_vanilla_hud,
                    ..AmmHostFrame::empty()
                });
            }
        }
        self.with(|runtime| Ok(runtime.frame(input)))
    }

    /// The size a `ResizeBuffers` or a `WM_SIZE` that asks for `asked` must use, and whether a render size is
    /// forced. It reads the lock-free state only, so the render and window threads never wait on the host lock;
    /// without a host nothing is forced.
    pub fn render_size_for(&self, asked: RenderSize) -> (bool, RenderSize) {
        let forced = self.shared.load().as_ref().and_then(|shared| shared.forced_size());
        (forced.is_some(), RenderSizeBook::size_for(forced, asked))
    }

    /// The size a `WM_SIZE` of `asked` gives the game, and whether a size is forced; a minimised window passes. It
    /// reads the lock-free state only, like `render_size_for`.
    pub fn window_size_for(&self, minimised: bool, asked: RenderSize) -> (bool, RenderSize) {
        let forced = self.shared.load().as_ref().and_then(|shared| shared.forced_size());
        RenderSizeBook::window_size_for(forced, minimised, asked)
    }

    /// The game window had a DPI, display or position change. While a size is forced the next frame sends it to
    /// the game again; with nothing forced it does nothing. It writes the lock-free state only, like
    /// `render_size_for`, so the window thread never waits on the host lock.
    pub fn window_changed(&self) {
        if let Some(shared) = self.shared.load().as_ref()
            && shared.forced_size().is_some()
        {
            shared.mark_window_changed();
        }
    }

    /// True while an active Input holder asks for the player's own turn to be blocked. It reads the lock-free
    /// state only, so the game thread never waits on the host lock; false without a host.
    pub fn blocks_player_turn(&self) -> bool {
        self.shared.load().as_ref().is_some_and(|shared| shared.blocks_player_turn())
    }

    /// True while the active exclusive Input holder has a `full_block` claim. It reads the lock-free state only, so
    /// the thread that polls the gamepad never waits on the host lock; false without a host.
    pub fn blocks_gamepad(&self) -> bool {
        self.shared.load().as_ref().is_some_and(|shared| shared.blocks_gamepad())
    }

    pub fn set_console(&self, callback: Option<AmmHostConsoleCallback>) {
        *self.console.lock().unwrap_or_else(PoisonError::into_inner) = callback;
    }

    /// The game-thread tick of the host, then the queued console lines go to the host mod.
    pub fn tick(&self, now_ms: u64) -> Result<(), AmmStatus> {
        self.with(|runtime| {
            runtime.tick(now_ms);
            Ok(())
        })?;
        let callback = *self.console.lock().unwrap_or_else(PoisonError::into_inner);
        if let (Some(callback), Some(logs)) = (callback, LogBridge::current()) {
            logs.drain_console(|level, line| {
                // SAFETY: the host mod gave a callback that takes a level and a text valid for the call, and keeps it
                // callable until it sets another one
                unsafe { callback(level as u8, line.as_ptr(), line.len()) }
            });
        }
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, Option<HostRuntime>> {
        self.runtime.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
