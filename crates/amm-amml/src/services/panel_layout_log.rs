use std::sync::atomic::{AtomicBool, Ordering};

use amml_api::{Logger, Size, Window};

use crate::types::PanelLayout;

/// One `panel layout` line per panel open: where the close button and the search field are, each as `x,y,w,h` in
/// the window's client pixels (the back-buffer rect scaled by client size over buffer size), so an in-game run can
/// click them with window messages. A control the panel does not draw is written `none`. When the window gives no
/// client or buffer size the rect is written in buffer pixels.
#[derive(Debug, Default)]
pub struct PanelLayoutLog {
    logged: AtomicBool,
}

impl PanelLayoutLog {
    pub fn new() -> Self {
        Self { logged: AtomicBool::new(false) }
    }

    /// Logs `layout` once for this open of the panel.
    pub fn on_open(&self, log: &Logger, layout: &PanelLayout, window: &Window) {
        if self.logged.swap(true, Ordering::Relaxed) {
            return;
        }
        let scale = Self::scale(window.client_size(), window.buffer_size());
        let close = Self::client_rect(layout.close, scale);
        let search = layout.search.map_or_else(|| "none".to_string(), |rect| Self::client_rect(rect, scale));
        log.info("panel layout", &[("close", &close), ("search", &search)]);
    }

    /// The panel closed: the next open logs its layout again.
    pub fn on_close(&self) {
        self.logged.store(false, Ordering::Relaxed);
    }

    /// Whether the layout of this open was logged.
    pub fn logged(&self) -> bool {
        self.logged.load(Ordering::Relaxed)
    }

    // client pixels per buffer pixel on each axis; 1 when a size is unknown or empty
    fn scale(client: Option<Size>, buffer: Option<Size>) -> (f32, f32) {
        match (client, buffer) {
            (Some(client), Some(buffer))
                if client.width > 0 && client.height > 0 && buffer.width > 0 && buffer.height > 0 =>
            {
                (client.width as f32 / buffer.width as f32, client.height as f32 / buffer.height as f32)
            }
            _ => (1.0, 1.0),
        }
    }

    // `[min_x, min_y, max_x, max_y]` in buffer pixels as `x,y,w,h` in client pixels, rounded
    fn client_rect([min_x, min_y, max_x, max_y]: [f32; 4], (scale_x, scale_y): (f32, f32)) -> String {
        let x = (min_x * scale_x).round();
        let y = (min_y * scale_y).round();
        let w = ((max_x - min_x) * scale_x).round();
        let h = ((max_y - min_y) * scale_y).round();
        format!("{x},{y},{w},{h}")
    }
}
