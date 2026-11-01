/// What a `FocusProbeHandler` does to its widget in the next pass.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum FocusAction {
    #[default]
    None,
    Request,
    Surrender,
}
