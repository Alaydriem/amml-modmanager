use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, PoisonError};

use amm_client::link::ManagerLink;
use amml_api::amml_abi::{
    ClientTick, GamepadPoll, InjectedInputReleased, InputLeaseLost, InputLeaseResumed, InputLeaseSuspended, KeyInput,
    LookDelta, ModsLoaded, MouseInput, PlayerMoveInput, ProviderGone, ProviderReady, ScreenshotTaken,
    ScreenshotWritten, Shutdown, WorldJoined, WorldLeft,
};
use amml_api::{
    AmmlMod, ApiError, Area, Ctx, DisableCtx, Event, EventCtx, Events, HookOrder, Listener, Logger, LookDeltaExt,
    MouseInputExt, PlayerMoveInputExt, ProviderGoneExt, Uses,
};

use crate::{ProbeChecks, ProbeLayer};

// The listeners of the enabled probe; `disable` drops them, which removes them from the loader.
static LISTENERS: Mutex<Vec<Listener>> = Mutex::new(Vec::new());
// The checks of the enabled probe, with the link they own.
static CHECKS: Mutex<Option<Arc<ProbeChecks>>> = Mutex::new(None);

/// The consumer test mod of the manager. `enable` starts a `ManagerLink` with the probe's descriptor and its egui
/// layer, and listens to the loader; `ProbeChecks` runs the checks from `ClientTick` and logs one line per step.
pub struct ProbeMod;

impl ProbeMod {
    /// The module string the loader knows this mod by: `@<NAMESPACE>/<NAME>`.
    pub const MODULE: &'static str = "@amml/managerprobe";

    /// The descriptor the probe registers with the manager.
    pub const DESCRIPTOR: &'static str = include_str!("../packaging/descriptor.hcl");

    // A listen the mod can do without: `None` when the loader cannot serve `E`.
    fn optional<E: Event>(
        events: Events,
        order: HookOrder,
        f: impl FnMut(&mut E, &EventCtx) + Send + 'static,
    ) -> Result<Option<Listener>, ApiError> {
        match events.listen::<E>(order, f) {
            Ok(listener) => Ok(Some(listener)),
            Err(ApiError::NotAvailable) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn listen_all(ctx: &Ctx, checks: &Arc<ProbeChecks>) -> Result<Vec<Listener>, ApiError> {
        let events = ctx.events();
        let mut listeners = Vec::new();
        let tick = Arc::clone(checks);
        listeners.push(events.listen::<ClientTick>(HookOrder::READ_AFTER_WRITERS, move |_, _| tick.tick())?);
        let gone = Arc::clone(checks);
        listeners.push(events.listen::<ProviderGone>(HookOrder::LAST, move |event, _| {
            if let Some(name) = event.name() {
                gone.provider_gone(name);
            }
        })?);
        let joined = Arc::clone(checks);
        listeners.extend(Self::optional::<WorldJoined>(events, HookOrder::READ_AFTER_WRITERS, move |_, _| {
            joined.world_joined();
        })?);
        let left = Arc::clone(checks);
        listeners.extend(Self::optional::<WorldLeft>(events, HookOrder::READ_AFTER_WRITERS, move |_, _| {
            left.world_left();
        })?);
        let moves = Arc::clone(checks);
        listeners.extend(Self::optional::<PlayerMoveInput>(events, HookOrder::LAST, move |event, _| {
            moves.move_input(event.intent());
        })?);
        let looks = Arc::clone(checks);
        listeners.extend(Self::optional::<LookDelta>(events, HookOrder::LAST, move |event, _| {
            let (injected, cancelled) = (event.is_injected(), event.is_cancelled());
            looks.look_delta(event.delta().yaw, injected, cancelled);
        })?);
        let mouse = Arc::clone(checks);
        listeners.extend(Self::optional::<MouseInput>(events, HookOrder::LAST, move |event, _| {
            mouse.mouse_input(event.action(), event.is_injected());
        })?);
        let gamepad = Arc::clone(checks);
        listeners.extend(Self::optional::<GamepadPoll>(events, HookOrder::LAST, move |event, _| {
            gamepad.gamepad_poll(event.is_cancelled());
        })?);
        let log = ctx.log();
        listeners.extend(Self::optional::<Shutdown>(events, HookOrder::NORMAL, move |_, _| {
            Self::shutdown(log);
        })?);
        Ok(listeners)
    }

    fn shutdown(log: Logger) {
        log.info("probe shutdown", &[]);
    }
}

impl AmmlMod for ProbeMod {
    const NAMESPACE: &'static str = "amml";
    const NAME: &'static str = "managerprobe";
    const USES: Uses = amml_api::amml_uses! {
        feature "core" {
            required ModsLoaded;
            required ClientTick;
            required WorldJoined;
            required WorldLeft;
            required ProviderReady;
            required ProviderGone;
            optional Shutdown;
        }
        feature "input" {
            required KeyInput;
            required area Area::INPUT_LEASE;
            optional PlayerMoveInput;
            optional LookDelta;
            optional MouseInput;
            optional GamepadPoll;
            optional InjectedInputReleased;
            optional InputLeaseSuspended;
            optional InputLeaseResumed;
            optional InputLeaseLost;
            optional area Area::KEY_INJECT;
            optional area Area::MOUSE_INJECT;
            optional area Area::TURN;
            optional area Area::MOVE;
        }
        feature "overlay" {
            optional ScreenshotTaken;
            optional ScreenshotWritten;
            optional area Area::OVERLAY_SCREENSHOT;
        }
    };

    fn enable(ctx: &Ctx) -> Result<(), ApiError> {
        let frames = Arc::new(AtomicU64::new(0));
        let layer = ProbeLayer::new(ctx.log(), Arc::clone(&frames));
        let link = ManagerLink::start(ctx, Self::DESCRIPTOR, Some(layer)).map_err(|error| {
            ctx.log().error("probe link failed", &[("error", &error.to_string())]);
            ApiError::Refused
        })?;
        let checks = ProbeChecks::new_shared(*ctx, frames, link);
        let listeners = Self::listen_all(ctx, &checks)?;
        *LISTENERS.lock().unwrap_or_else(PoisonError::into_inner) = listeners;
        *CHECKS.lock().unwrap_or_else(PoisonError::into_inner) = Some(checks);
        Ok(())
    }

    fn disable(_ctx: &Ctx, _how: &DisableCtx) {
        // The listeners go first, outside the lock, so none runs while the checks and their link close.
        let listeners = std::mem::take(&mut *LISTENERS.lock().unwrap_or_else(PoisonError::into_inner));
        drop(listeners);
        let checks = CHECKS.lock().unwrap_or_else(PoisonError::into_inner).take();
        drop(checks);
    }
}
