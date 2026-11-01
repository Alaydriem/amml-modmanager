use crate::structs::capture::CaptureStreamFailure;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LinkError {
    #[error("the mod manager is not loaded")]
    ManagerMissing,
    #[error("the mod manager speaks ABI major {found}")]
    AbiMismatch { found: u32 },
    #[error("the mod manager returned status {0}")]
    Status(i32),
    #[error("a JSON document is not valid: {reason}")]
    Json { reason: String },
    #[error("the icon file cannot be used: {reason}")]
    IconFile { reason: String },
    #[error("capture stream frame {} failed: {:?}", .0.tag, .0.fault)]
    StreamFrameFailed(CaptureStreamFailure),
}
