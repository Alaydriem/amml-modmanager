use amm_core::structs::settings::HclDocument;

/// What reading a config file found.
#[derive(Debug, Clone)]
pub enum ReadOutcome {
    Missing,
    Parsed(HclDocument),
    /// The file is not valid HCL text. It was copied to `<name>.invalid` before this value was returned.
    Invalid,
    /// The file exists but could not be read, or it is not valid HCL and its `.invalid` copy failed. It must not
    /// be overwritten, because nothing else keeps its text.
    Unreadable,
}
