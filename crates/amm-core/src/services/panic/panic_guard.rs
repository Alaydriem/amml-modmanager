use std::panic::{AssertUnwindSafe, catch_unwind};

/// Runs code that must not let a panic leave it, such as the body of an `extern "C"` function. The panic arm
/// (the caller's handler and the drop of the panic payload) runs inside a second `catch_unwind`, and a panic
/// there is forgotten, so nothing unwinds out even when the handler or the payload's drop panics.
pub struct PanicGuard;

impl PanicGuard {
    const UNKNOWN: &'static str = "unknown panic";

    /// The value of `f`, or `None` when it panicked. On a panic, `on_panic` gets the payload text.
    pub fn run<T>(f: impl FnOnce() -> T, on_panic: impl FnOnce(&str)) -> Option<T> {
        let payload = match catch_unwind(AssertUnwindSafe(f)) {
            Ok(value) => return Some(value),
            Err(payload) => payload,
        };
        let handled = catch_unwind(AssertUnwindSafe(move || {
            let text = payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| Self::UNKNOWN.to_string());
            on_panic(&text);
            drop(payload);
        }));
        if let Err(second) = handled {
            std::mem::forget(second);
        }
        None
    }
}
