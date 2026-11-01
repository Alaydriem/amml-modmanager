/// The widget a `FocusProbeHandler` shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusTarget {
    TextField,
    Button,
    /// A text field below a spacer in a short scroll area, so it is laid out out of view.
    ScrolledOutTextField,
}
