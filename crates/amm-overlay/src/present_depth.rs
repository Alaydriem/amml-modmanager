use std::cell::Cell;

thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// The Present nesting of this thread while the guard lives. Present1 can reach Present inside DXGI, and only the
/// outermost call draws.
pub(crate) struct PresentDepth {
    depth: u32,
}

impl PresentDepth {
    pub(crate) fn enter() -> Self {
        let depth = DEPTH.with(|cell| {
            let depth = cell.get() + 1;
            cell.set(depth);
            depth
        });
        Self { depth }
    }

    pub(crate) fn outermost(&self) -> bool {
        self.depth == 1
    }
}

impl Drop for PresentDepth {
    fn drop(&mut self) {
        DEPTH.with(|cell| cell.set(cell.get().saturating_sub(1)));
    }
}
