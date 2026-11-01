/// How disable ended for the manager host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisableOutcome {
    /// Every call into the host drained and the host was destroyed.
    Destroyed,
    /// The overlay or the window input did not drain in time, so a thread may still be inside a call into the
    /// host; the host is left alive and the next enable keeps it.
    KeptAlive,
}
