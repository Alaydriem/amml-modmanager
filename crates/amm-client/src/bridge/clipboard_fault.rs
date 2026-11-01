/// Why one clipboard access failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardFault {
    /// Another program holds the clipboard open; the access may succeed if tried again.
    Busy,
    /// The clipboard was open but the text could not be read or written.
    Failed,
}
