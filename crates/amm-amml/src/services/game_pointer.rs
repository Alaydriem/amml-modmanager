use amm_core::structs::render::RenderSize;
use amm_ffi::services::HostCell;
use amml_api::amml_abi::MouseInput;
use amml_api::{Event, Window};

/// Where the game's mouse feed puts the real cursor (`MouseInput`, `HookOrder::WRITE`): inside the game viewport of
/// the `Overlay` holder (`set_game_viewport`), the point of the full window it stands for; while a render size is
/// forced, every point in back-buffer pixels, because the game then sees its window at the back-buffer size. The
/// cursor is the last window pointer position the host was given (buffer coordinates). Otherwise the event is left
/// alone, as is an injected one. While no layer is drawn and no size is forced it reads neither the host lock nor the
/// window; the client size is read once and kept until the next window size or window change.
pub struct GamePointer {
    cell: &'static HostCell,
    window: Window,
}

impl GamePointer {
    pub fn new(cell: &'static HostCell, window: Window) -> Self {
        Self { cell, window }
    }

    pub fn on_mouse(&self, event: &mut MouseInput) {
        if event.is_injected() {
            return;
        }
        let window = &self.window;
        let client = || window.client_size().map(|size| RenderSize::new(size.width, size.height));
        if let Some((x, y)) = self.cell.game_point(client) {
            event.x = x.round() as i32;
            event.y = y.round() as i32;
        }
    }
}
