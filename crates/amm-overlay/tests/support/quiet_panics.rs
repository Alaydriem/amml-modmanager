use std::panic::PanicHookInfo;

type PanicHook = Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send + 'static>;

/// Silences the panic message of a panic a test causes on purpose; the previous hook is back on drop.
pub struct QuietPanics {
    previous: Option<PanicHook>,
}

impl QuietPanics {
    pub fn new() -> Self {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        Self { previous: Some(previous) }
    }
}

impl Drop for QuietPanics {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            std::panic::set_hook(previous);
        }
    }
}
