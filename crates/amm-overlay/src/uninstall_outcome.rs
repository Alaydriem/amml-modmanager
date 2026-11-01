/// How `WndProcRouter::uninstall` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallOutcome {
    /// The router was not installed. Nothing changed.
    NotInstalled,
    /// Routing stopped, no thread is inside the sink, and the window has its original procedure again.
    Restored,
    /// Routing stopped and no thread is inside the sink, but another procedure was set above ours. Ours stays in
    /// the chain and only passes messages on.
    PassThrough,
    /// A thread stayed inside the sink past the wait. Routing stopped, but the sink must stay alive.
    TimedOut,
}
