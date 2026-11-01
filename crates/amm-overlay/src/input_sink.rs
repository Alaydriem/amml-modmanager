use amm_abi::types::AmmCursorShape;
use amm_ffi::types::AmmHostPointerKind;

/// Where the window procedure sends the game window's input. Every call comes on the window thread, `cursor`
/// included, which is asked inside `WM_SETCURSOR`. A call that returns `true` consumed the message: the game's own
/// window procedure does not see it.
///
/// A call must not block, because the game's message loop waits for it.
pub trait InputSink: Sync {
    /// A pointer event in back-buffer pixels. `button` is 0 left, 1 right, 2 middle, and is read for `Button`;
    /// `wheel` is in notches and is read for `Wheel`.
    fn pointer(&self, kind: AmmHostPointerKind, x: f32, y: f32, button: u8, down: bool, wheel: f32) -> bool;

    /// One Unicode code point of text input. Surrogate pairs are already joined.
    fn char(&self, codepoint: u32) -> bool;

    /// The game window lost focus.
    fn focus_lost(&self);

    /// The size the game gets in `WM_SIZE`, given the `minimised` flag and the size the message carries, or
    /// `None` to pass the message on unchanged.
    fn window_size(&self, minimised: bool, width: u32, height: u32) -> Option<(u32, u32)>;

    /// The game window had a DPI, display or position change (`WM_DPICHANGED`, `WM_DISPLAYCHANGE`,
    /// `WM_WINDOWPOSCHANGED`). The message still reaches the game unchanged.
    fn window_changed(&self);

    /// Whether the cursor belongs to the layers (`true`) and the shape to show; asked on `WM_SETCURSOR` over the
    /// client area.
    fn cursor(&self) -> (bool, AmmCursorShape);
}
