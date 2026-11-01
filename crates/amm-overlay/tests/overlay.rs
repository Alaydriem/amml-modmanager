use std::time::Duration;

use amm_overlay::{Overlay, OverlayError, ShutdownOutcome};

use super::support::cross_thread::CrossThread;
use super::support::ref_count::RefCount;
use super::support::retour_installer::RetourInstaller;
use super::support::swap_window::SwapWindow;
use super::support::test_host::TestHost;

const TIMEOUT: Duration = Duration::from_millis(2000);
// shorter than the game's 2000 ms, to keep the test fast
const SHORT_TIMEOUT: Duration = Duration::from_millis(200);

#[test]
fn shutdown_with_no_call_in_flight_drains_and_releases_the_device() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    window.present();
    let held = RefCount::of(&window.device);
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
    assert!(RefCount::of(&window.device) < held, "the overlay's device references were released");
    window.present();
    assert_eq!(host.frames(), 1, "no frame after shutdown");
}

#[test]
fn shutdown_while_a_present_is_blocked_leaks_and_frees_nothing() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    window.present();
    let held = RefCount::of(&window.device);
    let gate = host.arm_gate();
    std::thread::scope(|scope| {
        let render = CrossThread(&window);
        let presenting = scope.spawn(move || {
            let render = render;
            render.0.present();
        });
        gate.entered.wait();
        assert_eq!(overlay.shutdown(SHORT_TIMEOUT), ShutdownOutcome::Leaked);
        assert_eq!(RefCount::of(&window.device), held, "nothing was released under the running call");
        gate.release.wait();
        presenting.join().unwrap();
    });
    assert_eq!(host.frames(), 2, "the blocked frame finished on the kept device");
    assert!(matches!(Overlay::install(RetourInstaller::new(), host, None), Err(OverlayError::Leaked)));
}
