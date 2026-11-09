use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use amm_abi::types::AmmStatus;
use amm_overlay::{ShutdownOutcome, UninstallOutcome};
use amml_sdk::SdkError;
use amml_sdk::amml_abi::AmmlLogLevel;

use super::ManagerState;
use crate::paths::ManagerPaths;
use crate::traits::GameCalls;
use crate::types::{DisableOutcome, HookGroup};

/// The order the manager starts and stops in. Enable starts the host before any hook, so no hook calls into a
/// host that does not run. Disable gives the game its window input and mouse back while the host still runs,
/// drains the overlay, stops the tick, raises the stop latch, puts the cursor and the HUD back, and destroys the
/// host only when no thread can still be inside a call into it.
pub struct ManagerLifecycle {
    state: &'static ManagerState,
    // a disable could not drain a call into the host; such a thread may never leave, so the host is never
    // destroyed in this process
    host_kept: AtomicBool,
}

impl ManagerLifecycle {
    /// How long disable waits for a thread inside a swap chain hook.
    pub const OVERLAY_TIMEOUT: Duration = Duration::from_millis(2000);
    /// The mouse grab and the HUD are put back only when the game ticked this recently.
    pub const RESTORE_TICK_WINDOW_MS: u64 = 1000;
    const KEPT_NOT_DRAINED: &'static str =
        "manager host left alive, the overlay or the window input did not stop in time";
    const KEPT_FROM_EARLIER: &'static str =
        "manager host left alive, an earlier disable could not drain a call into it";

    pub fn new(state: &'static ManagerState) -> Self {
        Self { state, host_kept: AtomicBool::new(false) }
    }

    pub fn enable(&self, calls: &dyn GameCalls, paths: &ManagerPaths) -> Result<(), SdkError> {
        let status = calls.create_host(paths);
        if status != AmmStatus::Ok {
            return Err(SdkError::Mod(format!("@amml/modmanager could not start its Rust host ({status:?})")));
        }
        calls.set_console(true);
        self.state.latch().lower();
        if let Err(error) = calls.install_hooks() {
            self.state.latch().raise();
            self.stop_host(calls, Self::KEPT_FROM_EARLIER);
            return Err(error);
        }
        calls.install_overlay();
        Ok(())
    }

    pub fn disable(&self, calls: &dyn GameCalls) -> DisableOutcome {
        calls.remove_hooks(HookGroup::Input);
        let input = calls.uninstall_wndproc();
        // the host forces no render size from here on; the restore is posted after the overlay drained, so no
        // frame can post a size message after it
        let restore = calls.close_render_size();
        calls.remove_hooks(HookGroup::Grab);
        calls.remove_hooks(HookGroup::Turn);
        // the overlay forgets the window at shutdown
        let foreground = calls.game_window_foreground();
        let overlay = calls.shutdown_overlay(Self::OVERLAY_TIMEOUT);
        if let Some(restore) = restore {
            calls.post_render_restore(restore);
        }
        calls.remove_hooks(HookGroup::World);
        // a game that stopped ticking is closing, and its ClientInstance and MinecraftGame may be gone
        let ticking = self.state.tick().ticked_within(calls.now_ms(), Self::RESTORE_TICK_WINDOW_MS);
        if ticking {
            // held fake keys and buttons go up while the game still ticks, since the keyboard and mouse calls need
            // its thread
            calls.release_fake_keys();
            self.state.fake_keys().flush(calls);
            // no look turn goes to the game here: the turn hook is already removed
            self.state.fake_pointer().flush(calls, 0);
        }
        calls.remove_hooks(HookGroup::Tick);
        self.state.latch().raise();
        if ticking {
            let client = self.state.tick().client();
            self.state.grab().restore(calls, client, foreground);
            self.state.hud().restore(calls, client);
        }
        if overlay == ShutdownOutcome::Leaked || input == UninstallOutcome::TimedOut {
            self.host_kept.store(true, Ordering::SeqCst);
        }
        self.stop_host(calls, Self::KEPT_NOT_DRAINED)
    }

    // `kept` is the log line when the host stays alive
    fn stop_host(&self, calls: &dyn GameCalls, kept: &str) -> DisableOutcome {
        if self.host_kept.load(Ordering::SeqCst) {
            calls.log(AmmlLogLevel::WARN, kept);
            return DisableOutcome::KeptAlive;
        }
        calls.set_console(false);
        calls.destroy_host();
        DisableOutcome::Destroyed
    }
}
