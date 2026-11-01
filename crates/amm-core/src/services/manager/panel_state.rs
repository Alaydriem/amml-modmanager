use crate::services::input::MenuButton;
use crate::structs::frame::MenuScreen;
use crate::structs::input::InputEvent;

/// The manager's own panel as routing and the snapshot see it: whether it is open, where the
/// pointer last was, and the menu button that opens it.
pub struct PanelState {
    open: bool,
    pointer: (f32, f32),
    menu_button: MenuButton,
    /// A mod Overlay layer covered the whole window in the last compose.
    /// It lags the layers by one frame: a layer's reply is read in the compose after it was recorded.
    overlay_covers: bool,
}

impl PanelState {
    pub fn new(menu_button_allowed: bool) -> Self {
        Self {
            open: false,
            pointer: (0.0, 0.0),
            menu_button: MenuButton::new(menu_button_allowed),
            overlay_covers: false,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens or closes the panel. Returns true when that changed it.
    pub fn set_open(&mut self, open: bool) -> bool {
        let changed = self.open != open;
        self.open = open;
        changed
    }

    pub fn pointer(&self) -> (f32, f32) {
        self.pointer
    }

    /// Keeps the pointer position of a pointer event.
    pub fn track(&mut self, ev: &InputEvent) {
        if let InputEvent::PointerMove { x, y } | InputEvent::PointerButton { x, y, .. } = *ev {
            self.pointer = (x, y);
        }
    }

    pub fn set_menu_screen(&mut self, screen: MenuScreen) {
        self.menu_button.set_screen(screen);
    }

    pub fn menu_screen(&self) -> MenuScreen {
        self.menu_button.screen()
    }

    pub fn menu_button_allowed(&self) -> bool {
        self.menu_button.allowed()
    }

    pub fn menu_button_visible(&self) -> bool {
        self.menu_button.visible(self.open, self.overlay_covers)
    }

    pub fn set_overlay_covers(&mut self, covers: bool) {
        self.overlay_covers = covers;
    }

    pub fn place_menu_button(&mut self, rect: Option<[f32; 4]>) {
        self.menu_button.place(rect);
    }

    pub fn menu_button_rect(&self) -> Option<[f32; 4]> {
        self.menu_button.rect(self.open, self.overlay_covers)
    }

    /// True when the menu button shows and the pointer is inside it.
    pub fn over_menu_button(&self) -> bool {
        self.menu_button.contains(self.open, self.overlay_covers, self.pointer)
    }
}
