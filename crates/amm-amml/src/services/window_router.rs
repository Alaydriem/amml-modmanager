use amm_abi::types::AmmCursorShape;
use amm_core::structs::input::InputEvent;
use amml_api::{CursorShape, Logger, MouseButton, Point, PointerKind, Window};

use super::{ManagerClock, ManagerInputSink, ManagerOverlayHost};

/// The game window's pointer, text and focus input, to the manager host, and the cursor shape back. Positions are
/// the loader's buffer coordinates (`WindowPointer.buffer`, mapped with the last swap chain size), the pixels the
/// panel and the layers lay out in, also while a render size is forced. A call that returns `true` cancels the
/// window message: the panel or a layer took it.
///
/// A focus loss is held back from the game while the panel is open, so the game does not pause under the panel;
/// once the panel closes, `release_focus` replays the held loss through `Window::replay_focus_loss`. The host
/// releases held key binds at every loss. A consumer's `full_block` input lease is the loader's, not the
/// manager's, so it holds no focus loss here; the loader releases its injected input at the loss.
pub struct WindowRouter {
    sink: ManagerInputSink,
    clock: ManagerClock,
    window: Window,
    log: Logger,
    host: &'static ManagerOverlayHost,
    // a focus loss was held back from the game and is not replayed yet
    focus_held: bool,
    // the shape of the last cursor line, so each change is logged once
    logged_shape: Option<CursorShape>,
}

impl WindowRouter {
    pub fn new(
        sink: ManagerInputSink,
        clock: ManagerClock,
        window: Window,
        log: Logger,
        host: &'static ManagerOverlayHost,
    ) -> Self {
        Self { sink, clock, window, log, host, focus_held: false, logged_shape: None }
    }

    /// One pointer event at `buffer`. `button` and `down` are read for `Button`, `notches` for `Wheel`. A leave,
    /// an unknown kind and the X buttons pass to the game.
    pub fn pointer(
        &mut self,
        kind: PointerKind,
        buffer: Point,
        button: Option<MouseButton>,
        down: bool,
        notches: f32,
    ) -> bool {
        let (x, y) = (buffer.x, buffer.y);
        let event = match kind {
            PointerKind::Move => InputEvent::PointerMove { x, y },
            PointerKind::Button => match button.and_then(Self::button_index) {
                Some(button) => InputEvent::PointerButton { button, down, x, y },
                None => return false,
            },
            PointerKind::Wheel => InputEvent::Wheel { delta: notches },
            PointerKind::Leave | PointerKind::Other => return false,
        };
        self.sink.route(event, self.clock.now_ms())
    }

    /// One character of text input. While the panel is open, each one logs `panel char received`.
    pub fn char(&mut self, ch: char) -> bool {
        let taken = self.sink.route(InputEvent::Char(u32::from(ch)), self.clock.now_ms());
        // the character itself is not logged: it may be text the operator types
        if self.sink.panel_open() {
            self.log.info("panel char received", &[("taken", if taken { "true" } else { "false" })]);
        }
        taken
    }

    /// The window lost focus (one of up to three messages of one loss). True holds the loss back from the game.
    pub fn focus_lost(&mut self) -> bool {
        self.sink.route(InputEvent::FocusLost, self.clock.now_ms());
        let hold = self.sink.panel_open();
        self.focus_held |= hold;
        hold
    }

    /// The window gained focus: the loader forgets the held losses, so nothing is left to replay.
    pub fn focus_gained(&mut self) {
        self.focus_held = false;
    }

    /// Replays a held focus loss once the panel is closed. Game thread, each tick.
    pub fn release_focus(&mut self) {
        if self.sink.panel_open() {
            return;
        }
        self.replay_held();
    }

    /// Replays a held focus loss now, panel open or not: the manager is disabled and holds nothing back any more.
    /// Game thread, while the game runs.
    pub fn replay_held(&mut self) {
        if !self.focus_held {
            return;
        }
        self.focus_held = false;
        match self.window.replay_focus_loss() {
            Ok(()) => self.log.info("focus loss replayed", &[]),
            Err(error) => self.log.warn("focus loss not replayed", &[("error", &error.to_string())]),
        }
    }

    /// The shape to show over the client area while the cursor belongs to the manager (the panel is open or a layer
    /// is in cursor mode): the shape the last frame asked for, an arrow when it asked for none. `None` leaves the
    /// cursor to the game. Each change of shape is logged once.
    pub fn cursor(&mut self) -> Option<CursorShape> {
        if !self.sink.wants(false).mouse_free {
            self.logged_shape = None;
            return None;
        }
        let shape = match self.host.cursor_shape() {
            AmmCursorShape::Hand => CursorShape::Hand,
            AmmCursorShape::Text => CursorShape::Text,
            AmmCursorShape::Default | AmmCursorShape::Arrow => CursorShape::Arrow,
        };
        if self.logged_shape.replace(shape) != Some(shape) {
            self.log.info("cursor shape", &[("shape", Self::shape_name(shape))]);
        }
        Some(shape)
    }

    // the host's button index: 0 left, 1 right, 2 middle; the X buttons have none
    fn button_index(button: MouseButton) -> Option<u8> {
        match button {
            MouseButton::Left => Some(0),
            MouseButton::Right => Some(1),
            MouseButton::Middle => Some(2),
            MouseButton::X1 | MouseButton::X2 => None,
        }
    }

    fn shape_name(shape: CursorShape) -> &'static str {
        match shape {
            CursorShape::Arrow => "Arrow",
            CursorShape::Hand => "Hand",
            CursorShape::Text => "Text",
        }
    }
}
