use std::cell::Cell;

use amm_core::services::PanicGuard;

struct PanickingPayload;

impl Drop for PanickingPayload {
    fn drop(&mut self) {
        panic!("payload drop failed on purpose")
    }
}

#[test]
fn a_panic_gives_none_and_the_handler_sees_the_payload_text() {
    let seen = Cell::new(String::new());

    let result: Option<u32> = PanicGuard::run(|| panic!("first failure"), |text| seen.set(text.to_string()));

    assert_eq!(result, None);
    assert_eq!(seen.take(), "first failure");
}

#[test]
fn a_payload_whose_drop_panics_does_not_unwind_out() {
    let handled = Cell::new(false);

    let result: Option<u32> = PanicGuard::run(|| std::panic::panic_any(PanickingPayload), |_| handled.set(true));

    assert_eq!(result, None);
    assert!(handled.get());
}

#[test]
fn a_handler_that_panics_does_not_unwind_out() {
    let result: Option<u32> = PanicGuard::run(|| panic!("first failure"), |_| panic!("handler failed on purpose"));

    assert_eq!(result, None);
}
