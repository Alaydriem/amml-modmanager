use amm_core::structs::render::RenderSize;
use amm_ffi::services::HostCell;
use amml_api::amml_abi::MouseInput;
use amml_api::{Event, Window};

/// Where the game's mouse feed puts the real cursor (`MouseInput`, `HookOrder::WRITE`): inside the game viewport of
/// the `Overlay` holder (`set_game_viewport`), the point of the full window it stands for; while a render size is
/// forced, every point in back-buffer pixels, because the game then sees its window at the back-buffer size. The
/// cursor is the last window pointer position the host was given (buffer coordinates). Otherwise the event is left
/// alone, as is an injected one.
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
        let Some(client) = self.window.client_size() else {
            return;
        };
        let client = RenderSize::new(client.width, client.height);
        let point = self.cell.with(|runtime| Ok(runtime.game_point(runtime.pointer(), client))).ok().flatten();
        if let Some((x, y)) = point {
            event.x = x.round() as i32;
            event.y = y.round() as i32;
        }
    }
}
