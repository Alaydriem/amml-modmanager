/// What the panel header says about saving: nothing, a save waiting for its merge window, a save
/// handed to the writer in the last moments, or a write the writer refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveStatus {
    Idle,
    Saving,
    Saved,
    Failed,
}
