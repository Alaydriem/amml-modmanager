use crate::in_flight::InFlight;

/// One thread inside a detour, counted for as long as the guard lives.
pub(crate) struct InFlightCall<'a> {
    count: &'a InFlight,
}

impl<'a> InFlightCall<'a> {
    pub(crate) fn new(count: &'a InFlight) -> Self {
        Self { count }
    }
}

impl Drop for InFlightCall<'_> {
    fn drop(&mut self) {
        self.count.leave();
    }
}
