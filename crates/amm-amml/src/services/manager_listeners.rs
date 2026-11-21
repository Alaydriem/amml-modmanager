use std::sync::Arc;

use amm_ffi::services::HostCell;
use amml_api::amml_abi::{
    ClientTick, KeyInput, ModsLoaded, MouseGrab, MouseInput, Shutdown, SwapChainResize, SwapChainResized,
    WindowChanged, WindowChar, WindowCursor, WindowFocus, WindowPointer, WindowSize, WorldJoined, WorldLeft,
};
use amml_api::{ApiError, Ctx, EventCtx, HookOrder, Listener, Logger};

use super::{GamePointer, ManagerClock, ManagerOverlayHost, ManagerState, PresentListener, RenderSizeControl};

/// The manager's listeners. They are added in `enable` only, and live until this value is dropped; the loader
/// removes them before `disable` runs, so a drop there only frees the closures. The closures hold the shared
/// `ManagerState`, the `PresentListener` and the `RenderSizeControl`; the hot ones (`KeyInput`, `MouseInput`,
/// `ClientTick`, `FramePresent`, `WindowPointer`, `WindowCursor`) do not allocate.
pub struct ManagerListeners {
    listeners: Vec<Listener>,
}

impl ManagerListeners {
    const CAPACITY: usize = 24;

    /// Adds every listener. A required event the loader cannot serve fails the call; a missing optional event
    /// (`Shutdown`, `MouseInput`, `MouseGrab`, `FramePresented`, `RenderDeviceReset`) is skipped; the loader logs
    /// the feature that is off.
    pub fn install(
        ctx: &Ctx,
        state: &Arc<ManagerState>,
        host: &'static ManagerOverlayHost,
        clock: ManagerClock,
    ) -> Result<ManagerListeners, ApiError> {
        let mut installed = Self { listeners: Vec::with_capacity(Self::CAPACITY) };
        installed.host(ctx, state)?;
        installed.window(ctx, state)?;
        let size = Arc::new(RenderSizeControl::new(HostCell::global(), ctx.window(), ctx.log()));
        installed.render_size(ctx, &size)?;
        Arc::new(PresentListener::new(ctx, host, clock, size)).listen(ctx, &mut installed)?;
        Ok(installed)
    }

    /// How many listeners are live.
    pub fn len(&self) -> usize {
        self.listeners.len()
    }

    pub fn is_empty(&self) -> bool {
        self.listeners.is_empty()
    }

    /// Keeps a listener the manager cannot run without: an error fails the enable.
    pub fn required(&mut self, listened: Result<Listener, ApiError>) -> Result<(), ApiError> {
        self.listeners.push(listened?);
        Ok(())
    }

    /// Keeps a listener of an optional event; `NotAvailable` skips it, any other error fails the enable.
    pub fn optional(&mut self, ctx: &Ctx, listened: Result<Listener, ApiError>) -> Result<(), ApiError> {
        match listened {
            Ok(listener) => {
                self.listeners.push(listener);
                Ok(())
            }
            Err(ApiError::NotAvailable) => Ok(()),
            Err(error) => {
                ctx.log().warn("listener not added", &[("error", &error.to_string())]);
                Err(error)
            }
        }
    }

    // The tick, the worlds, the keys, the game's mouse feed and grab, and the shutdown.
    fn host(&mut self, ctx: &Ctx, state: &Arc<ManagerState>) -> Result<(), ApiError> {
        let events = ctx.events();
        let log = ctx.log();
        let tick = Arc::clone(state);
        self.required(events.listen::<ClientTick>(HookOrder::NORMAL, move |_, at| tick.on_tick(at)))?;
        self.required(events.listen::<WorldJoined>(HookOrder::NORMAL, |_, _| Self::world(true)))?;
        self.required(events.listen::<WorldLeft>(HookOrder::NORMAL, |_, _| Self::world(false)))?;
        self.required(events.listen::<ModsLoaded>(HookOrder::NORMAL, move |_, _| {
            log.info("mods loaded", &[]);
        }))?;
        let keys = Arc::clone(state);
        self.required(events.listen::<KeyInput>(HookOrder::FIRST, move |event, _| keys.on_key(event)))?;

        let mouse = Arc::clone(state);
        self.optional(ctx, events.listen::<MouseInput>(HookOrder::FIRST, move |event, _| mouse.on_mouse(event)))?;
        let pointer = GamePointer::new(HostCell::global(), ctx.window());
        self.optional(ctx, events.listen::<MouseInput>(HookOrder::WRITE, move |event, _| pointer.on_mouse(event)))?;
        let grab = Arc::clone(state);
        self.optional(ctx, events.listen::<MouseGrab>(HookOrder::FIRST, move |event, _| grab.on_grab(event)))?;
        let shutdown = Arc::clone(state);
        self.optional(
            ctx,
            events.listen::<Shutdown>(HookOrder::NORMAL, move |_, at| Self::shutdown(&shutdown, at, log)),
        )?;
        Ok(())
    }

    // The game window's pointer, text, focus and cursor.
    fn window(&mut self, ctx: &Ctx, state: &Arc<ManagerState>) -> Result<(), ApiError> {
        let events = ctx.events();
        let pointer = Arc::clone(state);
        self.required(events.listen::<WindowPointer>(HookOrder::FIRST, move |event, _| pointer.on_pointer(event)))?;
        let text = Arc::clone(state);
        self.required(events.listen::<WindowChar>(HookOrder::FIRST, move |event, _| text.on_char(event)))?;
        let focus = Arc::clone(state);
        self.required(events.listen::<WindowFocus>(HookOrder::FIRST, move |event, _| focus.on_focus(event)))?;
        let cursor = Arc::clone(state);
        self.required(events.listen::<WindowCursor>(HookOrder::WRITE, move |event, _| cursor.on_cursor(event)))?;
        Ok(())
    }

    // The forced render size on the swap chain and the window size messages.
    fn render_size(&mut self, ctx: &Ctx, size: &Arc<RenderSizeControl>) -> Result<(), ApiError> {
        let events = ctx.events();
        let resize = Arc::clone(size);
        self.required(events.listen::<SwapChainResize>(HookOrder::WRITE, move |event, _| resize.on_resize(event)))?;
        let resized = Arc::clone(size);
        self.required(events.listen::<SwapChainResized>(HookOrder::NORMAL, move |event, _| resized.on_resized(event)))?;
        let window = Arc::clone(size);
        self.required(events.listen::<WindowSize>(HookOrder::WRITE, move |event, _| window.on_window_size(event)))?;
        let changed = Arc::clone(size);
        self.required(events.listen::<WindowChanged>(HookOrder::NORMAL, move |_, _| changed.on_window_changed()))?;
        Ok(())
    }

    // A world finished loading (`true`) or is being left (`false`).
    fn world(loaded: bool) {
        let _ = HostCell::global().with(|runtime| {
            runtime.world(loaded);
            Ok(())
        });
    }

    // The loader shuts down while the game still runs: the panel lease ends now, so its block ends before the
    // mods are disabled.
    fn shutdown(state: &ManagerState, at: &EventCtx, log: Logger) {
        drop(state.take_panel_lease());
        log.info("loader shutdown seen", &[("game_alive", if at.game_alive { "1" } else { "0" })]);
    }
}
