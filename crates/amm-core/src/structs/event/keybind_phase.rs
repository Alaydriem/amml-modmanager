#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeybindPhase {
    Pressed,
    Released,
    Repeat,
}
