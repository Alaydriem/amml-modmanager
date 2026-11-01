use std::cell::Cell;

use crate::in_flight::InFlight;
use crate::in_flight_call::InFlightCall;

thread_local! {
    // this thread's own scopes, so an uninstall run from inside a sink call does not wait for itself
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// One thread inside a call into the sink: counted for the uninstall drain, and on its own thread's depth.
pub(crate) struct CallScope<'a> {
    _call: InFlightCall<'a>,
}

impl<'a> CallScope<'a> {
    pub(crate) fn enter(calls: &'a InFlight) -> Self {
        DEPTH.with(|depth| depth.set(depth.get() + 1));
        Self { _call: calls.enter() }
    }

    /// The scopes the calling thread is in now.
    pub(crate) fn depth() -> usize {
        DEPTH.with(Cell::get)
    }
}

impl Drop for CallScope<'_> {
    fn drop(&mut self) {
        DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}
