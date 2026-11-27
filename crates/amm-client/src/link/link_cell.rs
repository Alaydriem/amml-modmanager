use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use amm_abi::types::{AmmCallbacks, AmmHandle, AmmStatus};
use amm_core::errors::LinkError;
use amm_core::structs::event::ModEvent;
use amm_core::structs::lease::AcquireOutcome;
use amml_api::{ApiError, Area, Ctx, KeyCode, MouseButton, MoveIntent};

use super::{EventDrain, InputLink, LinkShared, LinkState, ManagerTables, PointerOp};

/// The link's lock and the steps that change its state. Every loader and manager call runs with the lock released:
/// the loader raises its events on the calling thread, and a listener of the link takes the lock again. A step sets
/// the state it needs before it releases the lock (`connecting`, `Gone`), so an event during the call never sees a
/// half-made connect or teardown. Holds and leases taken out of the state are dropped after the lock is released.
#[derive(Clone)]
pub(crate) struct LinkCell {
    shared: Arc<Mutex<LinkShared>>,
}

impl LinkCell {
    pub(crate) fn new(shared: LinkShared) -> Self {
        Self { shared: Arc::new(Mutex::new(shared)) }
    }

    fn lock(&self) -> MutexGuard<'_, LinkShared> {
        self.shared.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn state(&self) -> LinkState {
        self.lock().state
    }

    pub(crate) fn ctx(&self) -> Ctx {
        self.lock().ctx
    }

    pub(crate) fn lease_id(&self) -> Option<u64> {
        self.lock().input.lease_id()
    }

    pub(crate) fn connection(&self) -> Result<(ManagerTables, AmmHandle), LinkError> {
        self.lock().connection()
    }

    /// A manager table was published: connect once all five are there.
    pub(crate) fn provider_ready(&self, name: &str) {
        if ManagerTables::is_manager_table(name) {
            self.try_connect();
        }
    }

    /// Connects when all five tables can be required now; otherwise the link keeps waiting.
    pub(crate) fn try_connect(&self) {
        let (ctx, descriptor, callbacks) = {
            let mut shared = self.lock();
            if !shared.may_connect() {
                return;
            }
            shared.connecting = true;
            (shared.ctx, shared.descriptor, shared.frame.as_ref().map(|frame| *frame.callbacks()))
        };
        let registered = Self::register(&ctx, descriptor);
        let (tables, handle) = match registered {
            Ok(Some(connection)) => connection,
            Ok(None) => {
                self.end_connect();
                return;
            }
            Err(error) => {
                self.lock().fault = Some(error.clone());
                self.end_connect();
                ctx.log().error("manager refused the registration", &[("error", &error.to_string())]);
                return;
            }
        };
        let given = callbacks.map(|callbacks| Self::set_callbacks(&ctx, &tables, handle, &callbacks));
        let gone = {
            let mut shared = self.lock();
            shared.connecting = false;
            if given == Some(true)
                && let Some(frame) = shared.frame.as_mut()
            {
                frame.mark_given();
            }
            if shared.state == LinkState::Gone {
                true
            } else {
                shared.connection = Some((tables, handle));
                shared.state = LinkState::Connected;
                false
            }
        };
        if gone {
            // The manager went, or the link closed, while the link registered: the registration is given back at once,
            // and the frame state that `close` left is freed on an `Ok`
            self.unregister(&ctx, &tables, handle);
            return;
        }
        ctx.log().info("manager connected", &[]);
    }

    // Ends a connect that made no registration. A link closed during it left its frame state to this step; the
    // manager never got the callbacks, so it is freed here.
    fn end_connect(&self) {
        let frame = {
            let mut shared = self.lock();
            shared.connecting = false;
            if shared.state == LinkState::Gone { shared.frame.take() } else { None }
        };
        if let Some(frame) = frame {
            // SAFETY: no registration was made, so the manager never got the callbacks
            unsafe { frame.free() };
        }
    }

    // `Ok(None)` while a table is missing.
    fn register(ctx: &Ctx, descriptor: &'static str) -> Result<Option<(ManagerTables, AmmHandle)>, LinkError> {
        let Ok(tables) = ManagerTables::require(&ctx.registry()) else {
            return Ok(None);
        };
        let register = tables.core.register.ok_or(LinkError::from(AmmStatus::Unsupported))?;
        let config_dir = ctx.dirs().config_dir().ok_or(LinkError::from(AmmStatus::InvalidArgument))?;
        let config_dir = config_dir.to_str().ok_or(LinkError::from(AmmStatus::InvalidArgument))?;
        let mut handle = AmmHandle { slot: u16::MAX, generation: u16::MAX };
        // SAFETY: the descriptor text, the folder text and the handle live for the call; the manager writes the
        // handle only on success
        let status = unsafe {
            register(descriptor.as_ptr(), descriptor.len(), config_dir.as_ptr(), config_dir.len(), &mut handle)
        };
        match status {
            AmmStatus::Ok => Ok(Some((tables, handle))),
            other => Err(LinkError::from(other)),
        }
    }

    // Whether the manager took the callbacks. A refusal keeps the frame state, which the manager never got, and is
    // logged: the mod draws nothing.
    fn set_callbacks(ctx: &Ctx, tables: &ManagerTables, handle: AmmHandle, callbacks: &AmmCallbacks) -> bool {
        let Some(set_callbacks) = tables.layers.set_callbacks else {
            ctx.log().error("manager has no set_callbacks, the layers are not drawn", &[]);
            return false;
        };
        // SAFETY: the callbacks live for the call and the manager copies them; the handle is this link's
        let status = unsafe { set_callbacks(handle, callbacks) };
        if status != AmmStatus::Ok {
            let status = (status as i32).to_string();
            ctx.log().error("manager refused the layer callbacks", &[("status", &status)]);
        }
        status == AmmStatus::Ok
    }

    /// Every mod is loaded: with no manager core table there is no manager, which is logged once and kept as the
    /// link's fault. A manager with a table missing is logged by name.
    pub(crate) fn mods_loaded(&self) {
        let ctx = {
            let shared = self.lock();
            if !shared.may_connect() {
                return;
            }
            shared.ctx
        };
        match ManagerTables::require(&ctx.registry()) {
            Err(missing) if missing == ManagerTables::NAMES[0] => {
                let kept = {
                    let mut shared = self.lock();
                    let kept = shared.may_connect();
                    if kept {
                        shared.fault = Some(LinkError::NoManager);
                    }
                    kept
                };
                if kept {
                    ctx.log().warn("manager not found", &[]);
                }
            }
            Err(missing) => ctx.log().warn("manager table not found", &[("table", missing)]),
            Ok(_) => self.try_connect(),
        }
    }

    /// A manager table is about to be dropped. The link is `Gone` before any call; then the events still queued are
    /// read, the mod is unregistered while the tables are still valid (the frame state is freed on `Ok`), the input
    /// lease ends, and `ManagerGone` is queued.
    pub(crate) fn provider_gone(&self, name: &str) {
        if !ManagerTables::is_manager_table(name) {
            return;
        }
        let (ctx, connection, holds, lease) = {
            let mut shared = self.lock();
            if shared.state == LinkState::Gone {
                return;
            }
            shared.state = LinkState::Gone;
            let holds = shared.input.take_holds();
            let lease = shared.input.take_lease();
            (shared.ctx, shared.connection.take(), holds, lease)
        };
        let mut events = VecDeque::new();
        if let Some((tables, handle)) = connection {
            EventDrain::poll(&tables, handle, &mut events);
            self.unregister(&ctx, &tables, handle);
        }
        drop(holds);
        drop(lease);
        {
            let mut shared = self.lock();
            shared.pending.extend(events);
            shared.pending.push_back(ModEvent::ManagerGone);
        }
        ctx.log().info("manager gone", &[]);
    }

    /// A loader event the link passes on: the manager events queued before it are read first, so the stream keeps
    /// arrival order.
    pub(crate) fn push(&self, event: ModEvent) {
        let polled = self.poll();
        let mut shared = self.lock();
        for polled in polled {
            shared.push_pending(polled);
        }
        shared.push_pending(event);
    }

    /// Every event not yet read, oldest first.
    pub(crate) fn take_events(&self) -> Vec<ModEvent> {
        let polled = self.poll();
        let mut shared = self.lock();
        for polled in polled {
            shared.push_pending(polled);
        }
        shared.pending.drain(..).collect()
    }

    fn poll(&self) -> VecDeque<ModEvent> {
        let mut polled = VecDeque::new();
        if let Ok((tables, handle)) = self.connection() {
            EventDrain::poll(&tables, handle, &mut polled);
        }
        polled
    }

    /// The loader released injected input of the link's lease: the hold of the one key or button it names, or every
    /// hold when it names none, is dropped (a release of a hold the loader released already does nothing), so the
    /// next down presses again.
    pub(crate) fn input_released(&self, key: Option<KeyCode>, button: Option<MouseButton>) {
        let holds = {
            let mut shared = self.lock();
            match (key, button) {
                (Some(key), _) => (shared.input.take_key(key).into_iter().collect(), Vec::new()),
                (None, Some(button)) => (Vec::new(), shared.input.take_button(button).into_iter().collect()),
                (None, None) => shared.input.take_holds(),
            }
        };
        drop(holds);
    }

    /// The link's lease is lost: its holds and the lease are dropped.
    pub(crate) fn input_lost(&self) {
        let (holds, lease) = {
            let mut shared = self.lock();
            (shared.input.take_holds(), shared.input.take_lease())
        };
        drop(holds);
        drop(lease);
    }

    /// Takes the input lease, or reports the one held: `Granted` while it is active, `Preempted` while another lease
    /// suspends it. A lost lease is taken again. A descriptor with no `Input` claim is `NotPermitted`; a request the
    /// loader refuses is `Api(Refused)`.
    pub(crate) fn acquire_input(&self) -> Result<AcquireOutcome, LinkError> {
        let (ctx, request, held) = {
            let mut shared = self.lock();
            let input = shared.input_mut()?;
            let request = input.request().ok_or(LinkError::from(AmmStatus::NotPermitted))?;
            let held = input.lease();
            (shared.ctx, request, held)
        };
        if !ctx.supports(Area::INPUT_LEASE) {
            return Err(LinkError::Api(ApiError::NotAvailable));
        }
        if let Some(outcome) = held.as_ref().and_then(|lease| InputLink::outcome(lease.state())) {
            return Ok(outcome);
        }
        drop(held);
        self.input_lost();
        let lease = Arc::new(ctx.input().acquire(request).map_err(LinkError::Api)?);
        let outcome = InputLink::outcome(lease.state()).unwrap_or(AcquireOutcome::Preempted);
        let (refused, rejected, other) = {
            let mut shared = self.lock();
            match shared.connection() {
                Err(error) => (Some(error), Some(lease), None),
                Ok(_) => {
                    let rejected = shared.input.set_lease(lease);
                    let other = rejected.as_ref().and(shared.input.lease());
                    (None, rejected, other)
                }
            }
        };
        // Ends a lease the link did not keep, with the lock released
        drop(rejected);
        match (refused, other) {
            (Some(error), _) => Err(error),
            // An event during the acquire took a lease first: that one is kept and reported
            (None, Some(kept)) => Ok(InputLink::outcome(kept.state()).unwrap_or(AcquireOutcome::Preempted)),
            (None, None) => Ok(outcome),
        }
    }

    /// Lets go of every key and button, then ends the lease.
    pub(crate) fn release_input(&self) {
        let parts = {
            let mut shared = self.lock();
            shared.input_mut().ok().map(|input| (input.take_holds(), input.take_lease()))
        };
        drop(parts);
    }

    /// Holds `key` down until a call with `down = false`. A second down of a held key and an up of a key not held
    /// change nothing.
    pub(crate) fn send_key(&self, key: KeyCode, down: bool) -> Result<(), LinkError> {
        if !down {
            let hold = self.lock().input_mut()?.take_key(key);
            drop(hold);
            return Ok(());
        }
        let lease = {
            let mut shared = self.lock();
            let input = shared.input_mut()?;
            if input.holds_key(key) {
                return Ok(());
            }
            input.lease_or_refused()?
        };
        let hold = lease.key_down(key).map_err(LinkError::Api)?;
        let extra = self.lock().input.put_key(lease.id(), key, hold);
        drop(extra);
        Ok(())
    }

    /// A button as `send_key` holds a key, a wheel turn, or a view turn.
    pub(crate) fn send_pointer(&self, op: PointerOp) -> Result<(), LinkError> {
        match op {
            PointerOp::Button { button, down: false } => {
                let hold = self.lock().input_mut()?.take_button(button);
                drop(hold);
                Ok(())
            }
            PointerOp::Button { button, down: true } => {
                let lease = {
                    let mut shared = self.lock();
                    let input = shared.input_mut()?;
                    if input.holds_button(button) {
                        return Ok(());
                    }
                    input.lease_or_refused()?
                };
                let hold = lease.button_down(button).map_err(LinkError::Api)?;
                let extra = self.lock().input.put_button(lease.id(), button, hold);
                drop(extra);
                Ok(())
            }
            PointerOp::Wheel { notches } => {
                let lease = self.lock().input_mut()?.lease_or_refused()?;
                lease.wheel(notches).map_err(LinkError::Api)
            }
            PointerOp::Turn(look) => {
                let lease = self.lock().input_mut()?.lease_or_refused()?;
                lease.turn(look).map_err(LinkError::Api)
            }
        }
    }

    /// Holds the movement until the next call. The all-zero intent clears it, and is `Ok` also without a lease.
    pub(crate) fn send_move(&self, intent: MoveIntent) -> Result<(), LinkError> {
        let lease = self.lock().input_mut()?.lease();
        if intent == MoveIntent::default() {
            return match lease {
                Some(lease) => lease.set_move(None).map_err(LinkError::Api),
                None => Ok(()),
            };
        }
        let lease = lease.ok_or(LinkError::Api(amml_api::ApiError::Refused))?;
        lease.set_move(Some(intent)).map_err(LinkError::Api)
    }

    /// Ends the link: unregisters while connected and ends the input lease. The frame state is freed when the
    /// manager no longer holds the callbacks; a state the manager may still call is kept.
    /// A link closed while a connect runs leaves its frame state to that connect, which may be giving the manager the
    /// callbacks at this moment: it frees the state once the registration is given back.
    pub(crate) fn close(&self) {
        let (ctx, connection, holds, lease, connecting) = {
            let mut shared = self.lock();
            shared.state = LinkState::Gone;
            let holds = shared.input.take_holds();
            let lease = shared.input.take_lease();
            (shared.ctx, shared.connection.take(), holds, lease, shared.connecting)
        };
        if let Some((tables, handle)) = connection {
            self.unregister(&ctx, &tables, handle);
        }
        drop(holds);
        drop(lease);
        if connecting {
            return;
        }
        let Some(frame) = self.lock().frame.take() else {
            return;
        };
        if frame.given() {
            // `FrameSlot` has no `Drop`, so the slot goes and the state behind it stays
            ctx.log().error("the manager may still call the layer callbacks, their frame state is kept", &[]);
        } else {
            // SAFETY: the manager never got the callbacks, so none runs with this state
            unsafe { frame.free() };
        }
    }

    // On `Ok` no callback runs after the call, so the frame state is freed. Any other status means the registration
    // may be live: the frame state stays and is kept rather than freed under a callback.
    fn unregister(&self, ctx: &Ctx, tables: &ManagerTables, handle: AmmHandle) {
        let status = match tables.core.unregister {
            // SAFETY: the handle is this link's; its connection was taken, so it is unregistered once
            Some(unregister) => unsafe { unregister(handle) },
            None => AmmStatus::Unsupported,
        };
        if status != AmmStatus::Ok {
            let status = (status as i32).to_string();
            ctx.log().error("unregister refused, the frame state is kept", &[("status", &status)]);
            return;
        }
        let frame = self.lock().frame.take();
        if let Some(frame) = frame {
            // SAFETY: unregister answered Ok, so the manager no longer calls the callbacks
            unsafe { frame.free() };
        }
    }
}
