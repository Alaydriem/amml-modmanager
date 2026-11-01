use std::sync::atomic::Ordering;
use std::time::Duration;

use amm_overlay::{Overlay, ShutdownOutcome};
use windows::Win32::Graphics::Dxgi::DXGI_PRESENT_TEST;

use super::support::d3d12_window::D3d12Window;
use super::support::quiet_panics::QuietPanics;
use super::support::retour_installer::RetourInstaller;
use super::support::swap_window::SwapWindow;
use super::support::test_host::TestHost;

const TIMEOUT: Duration = Duration::from_millis(2000);

#[test]
fn a_test_present_does_not_draw() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    window.present();
    assert_eq!(host.frames(), 1);
    window.present_with(DXGI_PRESENT_TEST);
    assert_eq!(host.frames(), 1);
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn present1_draws_once() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    window.present1();
    window.present1();
    assert_eq!(host.frames(), 2);
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn a_panicking_frame_still_presents_and_switches_the_overlay_off() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    window.present();
    assert_eq!(host.frames(), 1);
    let presented = window.present_count();
    host.panic_in_frame.store(true, Ordering::SeqCst);
    let quiet = QuietPanics::new();
    window.present();
    drop(quiet);
    assert_eq!(window.present_count(), presented + 1, "the original Present ran after the panic");
    host.panic_in_frame.store(false, Ordering::SeqCst);
    window.present();
    assert_eq!(window.present_count(), presented + 2);
    assert_eq!(host.frames(), 1, "the overlay is off after the panic");
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn the_game_can_resize_after_a_panic_switched_the_overlay_off() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    window.present();
    assert_eq!(host.frames(), 1);
    host.panic_in_frame.store(true, Ordering::SeqCst);
    let quiet = QuietPanics::new();
    window.present();
    drop(quiet);
    // ResizeBuffers fails while the overlay still holds a view of a back buffer
    window.resize(128, 96).unwrap();
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn the_game_can_resize_a_d3d12_chain_after_a_panic_switched_the_overlay_off() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = D3d12Window::new();
    window.submit();
    window.present();
    assert_eq!(host.frames(), 1);
    host.panic_in_frame.store(true, Ordering::SeqCst);
    let quiet = QuietPanics::new();
    window.present();
    drop(quiet);
    // ResizeBuffers fails while a D3D11On12 wrapper of a back buffer is left
    window.resize(128, 96).unwrap();
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn a_forced_size_replaces_the_resize_arguments_of_the_bound_chain() {
    let host = TestHost::leaked();
    host.set_ready();
    *host.forced_size.lock().unwrap() = Some((800, 450));
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    window.present();
    window.resize(0, 0).unwrap();
    assert_eq!(Overlay::<RetourInstaller>::back_buffer_size(), (800, 450));
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn a_chain_that_is_not_bound_keeps_its_resize_arguments() {
    let host = TestHost::leaked();
    host.set_ready();
    *host.forced_size.lock().unwrap() = Some((800, 450));
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let bound = SwapWindow::new();
    bound.present();
    let other = SwapWindow::new();
    other.resize(320, 200).unwrap();
    // SAFETY: the chain is live
    let desc = unsafe { other.chain.GetDesc() }.unwrap();
    assert_eq!((desc.BufferDesc.Width, desc.BufferDesc.Height), (320, 200));
    assert_eq!(host.render_size_asks(), 0);
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}
