use amm_core::structs::frame::FrameReplyData;

/// What one `FrameJob` came to once its callback ran and its mesh was checked and copied: not run (no frame
/// callback, or skipped after a failure that disabled the mod), failed (a bad status, reply or mesh), or drawn with
/// its reply and whether its geometry fit the mod's budgets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JobOutcome {
    NotRun,
    Failed,
    Drawn { reply: FrameReplyData, draw: bool },
}
