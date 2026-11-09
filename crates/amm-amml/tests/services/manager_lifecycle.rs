use std::sync::atomic::Ordering;

use amm_abi::types::{AMM_ABI_MAJOR, AmmCursorShape, AmmStatus};
use amm_client::link::{ApiTable, ManagerLink};
use amm_core::structs::input::{FakeKeyOp, PointerButton, PointerOp};
use amm_ffi::api::amm_api_for;
use amm_ffi::services::HostCell;
use amm_overlay::{ShutdownOutcome, UninstallOutcome};
use amml_modmanager::services::{ManagerRuntime, ManagerState};
use amml_modmanager::types::{DisableOutcome, HookGroup, RenderRestore};

use crate::support::host_dirs::HostDirs;
use crate::support::recording_calls::{Call, RecordingCalls};
use crate::support::test_mod::TestMod;

const LINK_DESCRIPTOR: &str = "namespace = \"test\"\nname = \"test-link\"\ndisplay_name = \"Test link\"\nversion = \"0.1.0\"\nauthor = \"A\"\nconfig_path = \"config.hcl\"\n";

struct Setup {
    dirs: HostDirs,
    state: &'static ManagerState,
    calls: &'static RecordingCalls,
    runtime: ManagerRuntime,
}

impl Setup {
    fn new(real_host: bool) -> Self {
        let dirs = HostDirs::new();
        let state = ManagerState::leaked();
        let calls = RecordingCalls::leaked(state, real_host);
        let runtime = ManagerRuntime::new(state, calls, dirs.paths());
        Self { dirs, state, calls, runtime }
    }

    const GAME: usize = 0x9000;

    // a tick in play at `now_ms` after a host frame that freed the cursor and hid the HUD: the mouse is released
    // and the HUD hidden
    fn take_mouse_and_hud(&self, now_ms: u64) {
        self.state.grab().keep_game(Self::GAME);
        self.state.frame().publish(self.state.latch(), true, AmmCursorShape::Default, true);
        self.calls.set_in_play(true);
        self.calls.set_now(now_ms);
        self.runtime.tick(RecordingCalls::CLIENT);
        let calls = self.calls.take();
        assert!(calls.contains(&Call::ReleaseMouse(Self::GAME)));
        assert!(calls.contains(&Call::SetHideGui(true)));
    }

    fn host_runs() -> bool {
        HostCell::global().with(|_| Ok(())).is_ok()
    }
}

#[test]
fn enable_starts_the_host_before_any_hook() {
    let setup = Setup::new(false);
    setup.state.latch().raise();

    setup.runtime.enable().unwrap();

    assert_eq!(
        setup.calls.take(),
        vec![
            Call::CreateHost,
            Call::SetConsole(true),
            Call::InstallHooks { latch_raised: false },
            Call::InstallOverlay
        ]
    );
}

#[test]
fn disable_drains_every_call_before_the_host_stops() {
    let setup = Setup::new(false);
    setup.runtime.enable().unwrap();
    setup.take_mouse_and_hud(5000);
    setup.calls.set_now(5500);

    assert_eq!(setup.runtime.disable(), DisableOutcome::Destroyed);

    assert_eq!(
        setup.calls.take(),
        vec![
            Call::RemoveHooks(HookGroup::Input),
            Call::UninstallWndproc,
            Call::CloseRenderSize,
            Call::RemoveHooks(HookGroup::Grab),
            Call::RemoveHooks(HookGroup::Turn),
            Call::WindowForeground,
            Call::ShutdownOverlay,
            Call::RemoveHooks(HookGroup::World),
            Call::ReleaseFakeKeys,
            Call::RemoveHooks(HookGroup::Tick),
            Call::GrabMouse { game: Setup::GAME, latch_raised: true },
            Call::SetHideGui(false),
            Call::SetConsole(false),
            Call::DestroyHost,
        ]
    );
}

#[test]
fn disable_releases_and_flushes_fake_keys_before_removing_hooks() {
    let setup = Setup::new(false);
    setup.runtime.enable().unwrap();
    setup.take_mouse_and_hud(5000);
    setup.calls.set_now(5500);
    setup.state.fake_keys().note(0x5000, 1);
    setup.calls.queue_fake_keys(&[FakeKeyOp { vk: 0x57, down: false }]);

    setup.runtime.disable();

    let calls = setup.calls.take();
    let at = |wanted: Call| calls.iter().position(|call| *call == wanted).unwrap();
    let release = at(Call::ReleaseFakeKeys);
    let up = at(Call::GameKey { controller: 0x5000, vk: 0x57, down: false, origin: 1 });
    assert!(release < up);
    assert!(up < at(Call::RemoveHooks(HookGroup::Tick)));
}

#[test]
fn disable_flushes_fake_buttons_before_removing_hooks() {
    let setup = Setup::new(false);
    setup.runtime.enable().unwrap();
    setup.take_mouse_and_hud(5000);
    setup.calls.set_now(5500);
    setup.state.fake_pointer().note(0x5100, 1, 2);
    setup.calls.queue_fake_pointer(&[PointerOp::Button { button: PointerButton::Left, down: false }]);

    setup.runtime.disable();

    let calls = setup.calls.take();
    let at = |wanted: Call| calls.iter().position(|call| *call == wanted).unwrap();
    let release = at(Call::ReleaseFakeKeys);
    let up = at(Call::GameMouse { device: 0x5100, action: 1, data: 0, x: 1, y: 2 });
    assert!(release < up);
    assert!(up < at(Call::RemoveHooks(HookGroup::Tick)));
}

#[test]
fn disable_sends_no_fake_keys_once_the_game_stopped_ticking() {
    let setup = Setup::new(false);
    setup.runtime.enable().unwrap();
    setup.take_mouse_and_hud(5000);
    setup.calls.set_now(6001);
    setup.state.fake_keys().note(0x5000, 1);
    setup.calls.queue_fake_keys(&[FakeKeyOp { vk: 0x57, down: false }]);

    setup.runtime.disable();

    let calls = setup.calls.take();
    assert!(!calls.contains(&Call::ReleaseFakeKeys));
    assert!(!calls.iter().any(|call| matches!(call, Call::GameKey { .. })));
}

#[test]
fn disable_closes_the_render_size_before_the_overlay_and_posts_the_restore_after_it() {
    let setup = Setup::new(false);
    setup.runtime.enable().unwrap();
    let restore = RenderRestore { window: 0x7000, width: 1280, height: 720 };
    *setup.calls.restore.lock().unwrap() = Some(restore);
    setup.calls.take();

    setup.runtime.disable();

    let calls = setup.calls.take();
    let at = |wanted: Call| calls.iter().position(|call| *call == wanted).unwrap();
    assert!(at(Call::UninstallWndproc) < at(Call::CloseRenderSize));
    assert!(at(Call::CloseRenderSize) < at(Call::ShutdownOverlay));
    assert!(at(Call::ShutdownOverlay) < at(Call::PostRenderRestore(restore)));
}

#[test]
fn disable_posts_the_restore_when_the_overlay_leaked() {
    let setup = Setup::new(false);
    setup.runtime.enable().unwrap();
    let restore = RenderRestore { window: 0x7000, width: 1280, height: 720 };
    *setup.calls.restore.lock().unwrap() = Some(restore);
    *setup.calls.overlay.lock().unwrap() = ShutdownOutcome::Leaked;
    setup.calls.take();

    setup.runtime.disable();

    assert!(setup.calls.take().contains(&Call::PostRenderRestore(restore)));
}

#[test]
fn disable_posts_no_restore_when_nothing_was_forced() {
    let setup = Setup::new(false);
    setup.runtime.enable().unwrap();
    setup.calls.take();

    setup.runtime.disable();

    let calls = setup.calls.take();
    assert!(calls.contains(&Call::CloseRenderSize));
    assert!(!calls.iter().any(|call| matches!(call, Call::PostRenderRestore(_))));
}

#[test]
fn the_mouse_stays_free_when_the_window_is_in_the_background() {
    let setup = Setup::new(false);
    setup.runtime.enable().unwrap();
    setup.take_mouse_and_hud(5000);
    setup.calls.foreground.store(false, Ordering::SeqCst);

    setup.runtime.disable();

    assert!(!setup.calls.take().iter().any(|call| matches!(call, Call::GrabMouse { .. })));
}

#[test]
fn nothing_is_restored_once_the_game_stopped_ticking() {
    let setup = Setup::new(false);
    setup.runtime.enable().unwrap();
    setup.take_mouse_and_hud(5000);
    setup.calls.set_now(6001);

    setup.runtime.disable();

    let calls = setup.calls.take();
    assert!(!calls.iter().any(|call| matches!(call, Call::GrabMouse { .. })), "no game call at shutdown");
    assert!(!calls.contains(&Call::SetHideGui(false)));
}

#[test]
fn nothing_is_restored_when_the_game_never_ticked() {
    let setup = Setup::new(false);
    setup.runtime.enable().unwrap();
    let state = setup.state;
    state.grab().keep_game(Setup::GAME);
    state.frame().publish(state.latch(), true, AmmCursorShape::Default, true);
    setup.calls.set_in_play(true);
    state.grab().apply(setup.calls, state.latch(), state.frame(), RecordingCalls::CLIENT);
    state.hud().apply(setup.calls, state.latch(), state.frame(), RecordingCalls::CLIENT);
    setup.calls.take();

    setup.runtime.disable();

    let calls = setup.calls.take();
    assert!(!calls.iter().any(|call| matches!(call, Call::GrabMouse { .. })));
    assert!(!calls.contains(&Call::SetHideGui(false)));
}

#[test]
fn a_router_that_did_not_drain_keeps_the_host_alive() {
    let setup = Setup::new(false);
    setup.runtime.enable().unwrap();
    *setup.calls.router.lock().unwrap() = UninstallOutcome::TimedOut;

    assert_eq!(setup.runtime.disable(), DisableOutcome::KeptAlive);

    let calls = setup.calls.take();
    assert!(!calls.contains(&Call::DestroyHost));
    assert!(!calls.contains(&Call::SetConsole(false)));
}

#[test]
fn a_leaked_overlay_keeps_the_host_and_the_next_enable_reuses_it() {
    let setup = Setup::new(true);
    setup.runtime.enable().unwrap();
    let test_mod = TestMod::register(&setup.dirs.mod_config("test-mod"));
    *setup.calls.overlay.lock().unwrap() = ShutdownOutcome::Leaked;

    assert_eq!(setup.runtime.disable(), DisableOutcome::KeptAlive);

    let calls = setup.calls.take();
    assert!(!calls.contains(&Call::DestroyHost));
    assert!(calls.iter().any(|call| matches!(call, Call::Log(line) if line.starts_with("manager host left alive"))));
    assert!(Setup::host_runs());

    setup.runtime.enable().unwrap();

    assert_eq!(setup.calls.take()[0], Call::CreateHost);
    assert_eq!(test_mod.unregister(), AmmStatus::Ok, "the mod registered before is still known to the host");
}

#[test]
fn a_mod_that_unregisters_after_the_host_is_destroyed_gets_manager_missing() {
    let setup = Setup::new(true);
    setup.runtime.enable().unwrap();
    let test_mod = TestMod::register(&setup.dirs.mod_config("test-mod"));
    // SAFETY: amm_api_for returns the manager's static table
    let table = unsafe { ApiTable::new(amm_api_for(AMM_ABI_MAJOR)) }.unwrap();
    let link = ManagerLink::without_frame(table, LINK_DESCRIPTOR, &setup.dirs.mod_config("test-link")).unwrap();

    assert_eq!(setup.runtime.disable(), DisableOutcome::Destroyed);
    assert!(!Setup::host_runs());

    assert_eq!(test_mod.unregister(), AmmStatus::ManagerMissing);
    drop(link);
}

#[test]
fn an_enable_that_fails_on_a_hook_after_a_leak_logs_why_the_host_stays() {
    let setup = Setup::new(false);
    setup.runtime.enable().unwrap();
    *setup.calls.overlay.lock().unwrap() = ShutdownOutcome::Leaked;
    setup.runtime.disable();
    setup.calls.fail_hooks.store(true, Ordering::SeqCst);
    setup.calls.take();

    assert!(setup.runtime.enable().is_err());

    let calls = setup.calls.take();
    assert!(!calls.contains(&Call::DestroyHost));
    let logs: Vec<&Call> = calls.iter().filter(|call| matches!(call, Call::Log(_))).collect();
    assert_eq!(
        logs,
        [&Call::Log("manager host left alive, an earlier disable could not drain a call into it".to_string())]
    );
}

#[test]
fn an_enable_that_fails_on_a_hook_stops_the_host_it_started() {
    let setup = Setup::new(false);
    setup.calls.fail_hooks.store(true, Ordering::SeqCst);

    assert!(setup.runtime.enable().is_err());

    let calls = setup.calls.take();
    assert_eq!(calls[calls.len() - 2..], [Call::SetConsole(false), Call::DestroyHost]);
    assert!(setup.state.latch().raised());
}
