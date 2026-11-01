use amm_core::structs::frame::LayerKind;

/// One job for the clipboard worker thread.
pub enum ClipboardRequest {
    /// Put this text on the clipboard.
    Copy(String),
    /// Read the clipboard and give the text to this layer.
    Paste(LayerKind),
}
