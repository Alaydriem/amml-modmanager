use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use amm_abi::types::AmmStatus;
use amm_core::services::PanicGuard;
use amm_core::services::render::RenderSizeBook;
use amm_core::structs::input::InputEvent;
use amm_core::structs::render::RenderSize;
use arc_swap::ArcSwapOption;

use super::{FrameCompositor, HostRuntime, HostShared, LockClock, LogBridge, ReentryGuard, TableGate};
use crate::types::{AmmHostConsoleCallback, AmmHostFrame, AmmHostFrameInput, AmmHostRoute};

/// The one runtime handle of the manager DLL: the host behind a lock, the state the render thread reads without the
/// lock, the table gate of the last host (kept after the host stops, so a late table call gets `Disabled`), and the
/// console callback of the host mod.
///
/// A Present takes the host lock twice, around the mods' frame callbacks but not across them, so input, tick and
/// table callers never wait for a mod's egui pass. The frame gate stands in for the lock while the callbacks run:
/// a Present holds it from start to end, and `unregister`, `set_callbacks` and `destroy` take it before the host
/// lock (`with_callbacks_held`), so a mod's callbacks stay valid while they run. Lock order: frame gate, then host
/// lock; nothing takes the gate while it holds the host lock.
pub struct HostCell {
    runtime: Mutex<Option<HostRuntime>>,
    frame_gate: Mutex<()>,
    shared: ArcSwapOption<HostShared>,
    gate: ArcSwapOption<TableGate>,
    console: Mutex<Option<AmmHostConsoleCallback>>,
    clock: LockClock,
    // the game window's client size packed `width << 32 | height`, or `UNKNOWN_SIZE` with the count of forgets when
    // unknown; forgotten at each window size or window change. A read stores its size only over the unknown value
    // it started from, so a size read across a forget is not kept
    client_size: AtomicU64,
    client_forgets: AtomicU64,
}

impl HostCell {
    const WIDTH_SHIFT: u32 = 32;
    // above every packed size: a client width never reaches 2^31
    const UNKNOWN_SIZE: u64 = 1 << 63;

    pub fn global() -> &'static HostCell {
        static CELL: OnceLock<HostCell> = OnceLock::new();
        CELL.get_or_init(|| Self {
            runtime: Mutex::new(None),
            frame_gate: Mutex::new(()),
            shared: ArcSwapOption::empty(),
            gate: ArcSwapOption::empty(),
            console: Mutex::new(None),
            clock: LockClock::new(),
            client_size: AtomicU64::new(Self::UNKNOWN_SIZE),
            client_forgets: AtomicU64::new(0),
        })
    }

    /// Starts the host, with its tables open. A host that already runs is kept.
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
        self.gate.store(Some(Arc::clone(created.table_gate())));
        *runtime = Some(created);
        Ok(())
    }

    /// Stops the host: no call reaches it after this begins, and pending writes are flushed within a bound. It waits
    /// for a running Present, whose frame callbacks may still run.
    pub fn destroy(&self) {
        let Some(_inside) = ReentryGuard::enter() else {
            return;
        };
        let _frame = self.lock_frame_gate();
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
        self.locked(f)
    }

    /// `with` for a call that changes or drops a mod's callbacks (`unregister`, `set_callbacks`): it first waits for
    /// a running Present, whose frame callbacks run without the host lock, so the old callbacks are not running
    /// when it returns.
    pub fn with_callbacks_held<T>(
        &self,
        f: impl FnOnce(&mut HostRuntime) -> Result<T, AmmStatus>,
    ) -> Result<T, AmmStatus> {
        let _inside = ReentryGuard::enter().ok_or(AmmStatus::NotPermitted)?;
        let _frame = self.lock_frame_gate();
        self.locked(f)
    }

    /// Places one input event. A pointer move that nothing can take (`HostShared::pointer_quiet`) passes without
    /// the host lock and is routed by the next call that takes it, so the tracked pointer stays as if it had been
    /// routed at once.
    pub fn route(&self, event: InputEvent, now_ms: u64) -> Result<AmmHostRoute, AmmStatus> {
        if let InputEvent::PointerMove { x, y } = event
            && let Some(shared) = self.shared.load().as_ref()
            && shared.pointer_quiet()
        {
            shared.set_pending_pointer(x, y);
            return Ok(AmmHostRoute::PASS);
        }
        self.with(|runtime| Ok(runtime.route(event, now_ms)))
    }

    /// Where the game's mouse feed must put the real cursor, or `None` to leave the game's own point
    /// (`HostRuntime::game_point` at the tracked pointer). While no layer is drawn and no size is forced it answers
    /// `None` from the lock-free state, and `client` is not asked for. `client` gives the window's client size,
    /// read once and kept until the next window size or window change; `None` there leaves the game's point.
    pub fn game_point(&self, client: impl FnOnce() -> Option<RenderSize>) -> Option<(f32, f32)> {
        let shared = self.shared.load_full()?;
        if shared.game_point_idle() {
            return None;
        }
        let client = self.client_size(client)?;
        self.with(|runtime| Ok(runtime.game_point(runtime.pointer(), client))).ok().flatten()
    }

    /// Forgets the kept client size of the game window; the next `game_point` that needs it reads it again.
    pub fn forget_client_size(&self) {
        let forgets = self.client_forgets.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.client_size.store(Self::UNKNOWN_SIZE | (forgets & !Self::UNKNOWN_SIZE), Ordering::Relaxed);
    }

    /// True once the last host closed its tables (`HostRuntime::disable_tables`): every table function then answers
    /// `Disabled`. False before the first host. It reads the lock-free state only.
    pub fn tables_closed(&self) -> bool {
        self.gate.load().as_ref().is_some_and(|gate| gate.is_closed())
    }

    /// The lock-free host state, while a host runs.
    pub fn shared(&self) -> Option<Arc<HostShared>> {
        self.shared.load_full()
    }

    /// Composes one Present. With no layer, no toast, the panel closed, no menu button to show or take away and no
    /// render size work it answers from the lock-free state. Otherwise it takes the host lock to start the frame,
    /// runs the mods' frame callbacks without it, and takes it again to build the layers; the frame gate is held
    /// throughout. A table call from inside a callback is still refused (`ReentryGuard`).
    pub fn frame(&self, input: &AmmHostFrameInput) -> Result<AmmHostFrame, AmmStatus> {
        {
            let shared = self.shared.load();
            let shared = shared.as_ref().ok_or(AmmStatus::ManagerMissing)?;
            shared.set_back_buffer(RenderSize::new(input.width, input.height), input.format);
            let snapshot = shared.snapshot().load();
            let button = snapshot.menu_button && (input.menu_screen != 0 || shared.menu_button_placed());
            let idle = snapshot.layers.is_empty() && !snapshot.panel_open && !button && !shared.render_size_busy();
            if idle && !shared.toast_visible(input.now_ms) {
                return Ok(AmmHostFrame {
                    freed_owners: shared.take_freed(),
                    cursor_free: snapshot.cursor_free,
                    hide_vanilla_hud: snapshot.hide_vanilla_hud,
                    ..AmmHostFrame::empty()
                });
            }
        }
        self.frame_with_step(input, |compositor, input| {
            // SAFETY: `frame_with_step` holds the frame gate while it runs the step, and `unregister`,
            // `set_callbacks` and `destroy` take it before they change a mod's callbacks
            unsafe { compositor.run_callbacks(input) }
        })
    }

    /// `frame` with `step` run in place of the mods' frame callbacks, between the two lock holds, with the frame
    /// gate held. `frame` passes `FrameCompositor::run_callbacks`; tests pass other steps. A panic in `step` is
    /// caught and logged: the compositor goes back to the host and the frame ends with what it has. When the frame
    /// has no callback to run, it starts and ends under one hold of the lock and `step` does not run.
    pub fn frame_with_step(
        &self,
        input: &AmmHostFrameInput,
        step: impl FnOnce(&mut FrameCompositor, &AmmHostFrameInput),
    ) -> Result<AmmHostFrame, AmmStatus> {
        let _inside = ReentryGuard::enter().ok_or(AmmStatus::NotPermitted)?;
        let _frame = self.lock_frame_gate();
        let mut start = {
            let mut runtime = self.lock();
            let started = self.clock.start();
            let runtime = runtime.as_mut().ok_or(AmmStatus::ManagerMissing)?;
            let start = runtime.begin_frame(input);
            if !start.callbacks {
                let frame = runtime.end_frame(start, input);
                runtime.publish_shared();
                if let Some(started) = started {
                    self.clock.record_hold(started.elapsed());
                }
                return Ok(frame);
            }
            runtime.publish_shared();
            (start, started.map(|started| started.elapsed()))
        };
        if let Some(compositor) = start.0.compositor.as_mut() {
            Self::run_step(compositor, input, step);
        }
        let (start, held) = start;
        let mut runtime = self.lock();
        let restarted = self.clock.start();
        let runtime = runtime.as_mut().ok_or(AmmStatus::ManagerMissing)?;
        let frame = runtime.end_frame(start, input);
        runtime.publish_shared();
        if let (Some(held), Some(restarted)) = (held, restarted) {
            self.clock.record_hold(held + restarted.elapsed());
        }
        Ok(frame)
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
        self.forget_client_size();
        if let Some(shared) = self.shared.load().as_ref()
            && shared.forced_size().is_some()
        {
            shared.mark_window_changed();
        }
    }

    pub fn set_console(&self, callback: Option<AmmHostConsoleCallback>) {
        *self.console.lock().unwrap_or_else(PoisonError::into_inner) = callback;
    }

    /// The game-thread tick of the host, then the queued console lines go to the host mod. While telemetry is on,
    /// a `perf` line of the host lock times is logged once per `LockClock::WINDOW_MS`.
    pub fn tick(&self, now_ms: u64) -> Result<(), AmmStatus> {
        self.with(|runtime| {
            runtime.tick(now_ms);
            Ok(())
        })?;
        if let Some(window) = self.clock.close_window(now_ms) {
            curia::info!("perf", {
                presents: window.presents,
                lock_hold_mean_us: window.hold_mean_us,
                lock_hold_max_us: window.hold_max_us,
                lock_waits: window.waits,
                lock_wait_mean_us: window.wait_mean_us,
                lock_wait_max_us: window.wait_max_us,
            });
        }
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

    // Runs `f` under the host lock and publishes the lock-free state. The caller holds the `ReentryGuard`. A pointer
    // move that passed without the lock is routed first. While telemetry is on the wait for the lock is timed.
    fn locked<T>(&self, f: impl FnOnce(&mut HostRuntime) -> Result<T, AmmStatus>) -> Result<T, AmmStatus> {
        let started = self.clock.start();
        let mut runtime = self.lock();
        if let Some(started) = started {
            self.clock.record_wait(started.elapsed());
        }
        let runtime = runtime.as_mut().ok_or(AmmStatus::ManagerMissing)?;
        runtime.route_pending();
        let result = f(runtime);
        runtime.publish_shared();
        self.clock.set_enabled(runtime.manager().config().telemetry);
        result
    }

    fn client_size(&self, read: impl FnOnce() -> Option<RenderSize>) -> Option<RenderSize> {
        let packed = self.client_size.load(Ordering::Relaxed);
        if packed & Self::UNKNOWN_SIZE == 0 {
            return Some(RenderSize::new((packed >> Self::WIDTH_SHIFT) as u32, packed as u32));
        }
        let size = read().filter(|size| !size.is_empty())?;
        let known = (u64::from(size.width) << Self::WIDTH_SHIFT) | u64::from(size.height);
        if known & Self::UNKNOWN_SIZE == 0 {
            let _ = self.client_size.compare_exchange(packed, known, Ordering::Relaxed, Ordering::Relaxed);
        }
        Some(size)
    }

    // Runs the unlocked step of a frame. A panic is caught here, so the compositor stays in the frame and goes back
    // to the host at `end_frame`.
    fn run_step(
        compositor: &mut FrameCompositor,
        input: &AmmHostFrameInput,
        step: impl FnOnce(&mut FrameCompositor, &AmmHostFrameInput),
    ) {
        let _ = PanicGuard::run(
            || step(compositor, input),
            |reason| curia::error!("frame callbacks panicked, the frame ends with what it has", { reason: reason }),
        );
    }

    fn lock(&self) -> MutexGuard<'_, Option<HostRuntime>> {
        self.runtime.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_frame_gate(&self) -> MutexGuard<'_, ()> {
        self.frame_gate.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
