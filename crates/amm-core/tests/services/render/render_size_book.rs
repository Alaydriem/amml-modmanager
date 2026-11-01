use amm_core::errors::ManagerError;
use amm_core::services::lease::LeaseArbiter;
use amm_core::services::render::RenderSizeBook;
use amm_core::structs::registry::ModIndex;
use amm_core::structs::render::RenderSize;

const HOLDER: ModIndex = ModIndex(0);
const OTHER: ModIndex = ModIndex(1);
const WINDOW: RenderSize = RenderSize { width: 1600, height: 900 };
const UHD: RenderSize = RenderSize { width: 3840, height: 2160 };
const MINIMISED: RenderSize = RenderSize { width: 0, height: 0 };

struct BookRig;

impl BookRig {
    // a book that has seen one frame of the window, so a later frame with the same size is no resize
    fn seen() -> RenderSizeBook {
        let mut book = RenderSizeBook::new();
        book.frame(WINDOW, 0);
        book
    }
}

#[test]
fn a_forced_size_is_sent_once_on_the_next_frame() {
    let mut book = BookRig::seen();

    assert_eq!(book.set(HOLDER, Some(UHD)), Ok(()));

    assert_eq!(book.frame(WINDOW, 0), Some(UHD));
    assert_eq!(book.frame(WINDOW, 0), None);
    assert_eq!(book.forced(), Some(UHD));
}

#[test]
fn a_window_resize_while_forced_sends_the_forced_size_again() {
    let mut book = BookRig::seen();
    book.set(HOLDER, Some(UHD)).ok();
    book.frame(WINDOW, 0);

    assert_eq!(book.frame(RenderSize::new(1280, 720), 0), Some(UHD));
    assert_eq!(book.frame(RenderSize::new(1280, 720), 0), None);
}

#[test]
fn ending_the_size_sends_the_client_size_of_the_frame() {
    let mut book = BookRig::seen();
    book.set(HOLDER, Some(UHD)).ok();
    book.frame(WINDOW, 0);

    book.set(HOLDER, None).ok();

    assert_eq!(book.forced(), None);
    assert!(book.busy());
    assert_eq!(book.frame(RenderSize::new(1280, 720), 0), Some(RenderSize::new(1280, 720)));
    assert!(!book.busy());
    assert_eq!(book.frame(RenderSize::new(1280, 720), 0), None);
}

#[test]
fn a_minimised_window_holds_the_restore_until_it_has_a_size() {
    let mut book = BookRig::seen();
    book.set(HOLDER, Some(UHD)).ok();
    book.frame(WINDOW, 0);
    book.set(HOLDER, None).ok();

    assert_eq!(book.frame(MINIMISED, 0), None);
    assert_eq!(book.frame(MINIMISED, 0), None);
    assert!(book.busy());
    assert_eq!(book.frame(WINDOW, 0), Some(WINDOW));
}

#[test]
fn a_minimised_window_holds_the_forced_size_until_it_has_a_size() {
    let mut book = RenderSizeBook::new();
    book.frame(MINIMISED, 0);
    book.set(HOLDER, Some(UHD)).ok();

    assert_eq!(book.frame(MINIMISED, 0), None);
    assert_eq!(book.frame(WINDOW, 0), Some(UHD));
}

#[test]
fn none_from_a_mod_that_forces_nothing_keeps_the_forced_size() {
    let mut book = BookRig::seen();
    book.set(HOLDER, Some(UHD)).ok();
    book.frame(WINDOW, 0);

    assert_eq!(book.set(OTHER, None), Ok(()));

    assert_eq!(book.forced(), Some(UHD));
    assert_eq!(book.frame(WINDOW, 0), None);
}

#[test]
fn a_side_of_zero_or_past_the_texture_limit_is_refused_and_changes_nothing() {
    let mut book = BookRig::seen();

    assert_eq!(book.set(HOLDER, Some(RenderSize::new(0, 2160))), Err(ManagerError::RenderSizeOutOfRange));
    assert_eq!(book.set(HOLDER, Some(RenderSize::new(16385, 2160))), Err(ManagerError::RenderSizeOutOfRange));
    assert_eq!(book.set(HOLDER, Some(RenderSize::new(16384, 16384))), Ok(()));
    assert_eq!(book.forced(), Some(RenderSize::new(16384, 16384)));
}

#[test]
fn only_the_holder_reads_its_wanted_size() {
    let mut book = BookRig::seen();
    book.set(HOLDER, Some(UHD)).ok();

    assert_eq!(book.state(HOLDER, WINDOW, 28).wanted, Some(UHD));
    assert_eq!(book.state(OTHER, WINDOW, 28).wanted, None);
}

#[test]
fn a_resize_takes_the_forced_size_while_one_is_in_effect() {
    let asked = RenderSize::new(1280, 720);

    assert_eq!(RenderSizeBook::size_for(Some(UHD), asked), UHD);
    assert_eq!(RenderSizeBook::size_for(None, asked), asked);
}

#[test]
fn a_set_after_shutdown_is_refused_and_forces_nothing() {
    let mut book = BookRig::seen();
    book.set(HOLDER, Some(RenderSize::new(800, 450))).ok();
    book.frame(WINDOW, 0);

    assert_eq!(book.shutdown(RenderSize::new(1280, 720)), Some(RenderSize::new(1280, 720)));

    assert_eq!(book.set(HOLDER, Some(RenderSize::new(800, 450))), Err(ManagerError::RenderSizeClosed));
    assert_eq!(book.forced(), None);
    assert_eq!(book.frame(RenderSize::new(1280, 720), 0), None);
}

#[test]
fn a_none_after_shutdown_is_a_no_op() {
    let mut book = BookRig::seen();
    book.shutdown(WINDOW);

    assert_eq!(book.set(HOLDER, None), Ok(()));
    assert!(!book.busy());
    assert_eq!(book.frame(WINDOW, 0), None);
}

#[test]
fn shutdown_with_a_minimised_window_still_ends_the_forced_size() {
    let mut book = BookRig::seen();
    book.set(HOLDER, Some(RenderSize::new(800, 450))).ok();
    book.frame(WINDOW, 0);

    assert_eq!(book.shutdown(MINIMISED), None);

    assert_eq!(book.forced(), None);
    assert!(!book.busy());
    assert_eq!(book.frame(RenderSize::new(1280, 720), 0), None);
}

#[test]
fn no_message_after_shutdown_even_when_the_holder_drops_its_lease() {
    let mut book = BookRig::seen();
    book.set(HOLDER, Some(UHD)).ok();
    book.frame(WINDOW, 0);
    book.shutdown(WINDOW);

    book.drop_unleased(&LeaseArbiter::new());

    assert_eq!(book.frame(WINDOW, 0), None);
    assert_eq!(book.frame(RenderSize::new(1280, 720), 0), None);
}

#[test]
fn a_reopened_book_forces_a_size_again() {
    let mut book = BookRig::seen();
    book.shutdown(WINDOW);
    assert_eq!(book.set(HOLDER, Some(UHD)), Err(ManagerError::RenderSizeClosed));

    book.open();

    assert_eq!(book.set(HOLDER, Some(UHD)), Ok(()));
    assert_eq!(book.frame(WINDOW, 0), Some(UHD));
}

#[test]
fn a_forced_size_is_sent_again_once_the_keep_alive_passes() {
    let mut book = BookRig::seen();
    book.set(HOLDER, Some(UHD)).ok();
    assert_eq!(book.frame(WINDOW, 5000), Some(UHD));

    assert_eq!(book.frame(WINDOW, 5000 + RenderSizeBook::KEEP_ALIVE_MS - 1), None);
    assert_eq!(book.frame(WINDOW, 5000 + RenderSizeBook::KEEP_ALIVE_MS), Some(UHD));
    assert_eq!(book.frame(WINDOW, 5000 + RenderSizeBook::KEEP_ALIVE_MS + 1), None);
}

#[test]
fn a_resend_for_any_reason_restarts_the_keep_alive() {
    let mut book = BookRig::seen();
    book.set(HOLDER, Some(UHD)).ok();
    book.frame(WINDOW, 5000);

    assert_eq!(book.frame(RenderSize::new(1280, 720), 5600), Some(UHD));

    assert_eq!(book.frame(RenderSize::new(1280, 720), 6000), None);
    assert_eq!(book.frame(RenderSize::new(1280, 720), 6600), Some(UHD));
}

#[test]
fn a_window_change_while_forced_sends_the_forced_size_on_the_next_frame() {
    let mut book = BookRig::seen();
    book.set(HOLDER, Some(UHD)).ok();
    book.frame(WINDOW, 5000);

    book.window_changed();

    assert_eq!(book.frame(WINDOW, 5016), Some(UHD));
    assert_eq!(book.frame(WINDOW, 5032), None);
}

#[test]
fn a_window_change_while_minimised_waits_for_a_frame_with_a_size() {
    let mut book = BookRig::seen();
    book.set(HOLDER, Some(UHD)).ok();
    book.frame(WINDOW, 5000);

    book.window_changed();

    assert_eq!(book.frame(MINIMISED, 5016), None);
    assert_eq!(book.frame(WINDOW, 5032), Some(UHD));
}

#[test]
fn with_nothing_forced_neither_trigger_sends_a_message() {
    let mut book = BookRig::seen();

    book.window_changed();

    assert_eq!(book.frame(WINDOW, 5000), None);
    assert_eq!(book.frame(WINDOW, 5000 + 10 * RenderSizeBook::KEEP_ALIVE_MS), None);
    assert!(!book.busy());
}

#[test]
fn a_window_change_after_the_size_ended_sends_only_the_restore() {
    let mut book = BookRig::seen();
    book.set(HOLDER, Some(UHD)).ok();
    book.frame(WINDOW, 5000);
    book.set(HOLDER, None).ok();

    book.window_changed();

    assert_eq!(book.frame(WINDOW, 5016), Some(WINDOW));
    assert_eq!(book.frame(WINDOW, 9000), None);
}

#[test]
fn a_closed_book_ignores_a_window_change() {
    let mut book = BookRig::seen();
    book.set(HOLDER, Some(UHD)).ok();
    book.frame(WINDOW, 5000);
    book.shutdown(WINDOW);

    book.window_changed();

    assert_eq!(book.frame(WINDOW, 9000), None);
}
