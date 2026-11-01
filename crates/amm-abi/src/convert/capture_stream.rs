use amm_core::structs::capture::{
    CaptureStreamFailure, CaptureStreamFault, CaptureStreamFrame, CaptureStreamRequest, LentStreamFrame,
};

use amm_core::traits::AbiSized;

use crate::types::{AmmCaptureStreamFault, AmmCaptureStreamFrame, AmmCaptureStreamRequest};

impl From<CaptureStreamRequest> for AmmCaptureStreamRequest {
    fn from(value: CaptureStreamRequest) -> Self {
        Self { tag: value.tag, present_seq: value.present_seq, ..Self::blank() }
    }
}

impl From<AmmCaptureStreamRequest> for CaptureStreamRequest {
    fn from(value: AmmCaptureStreamRequest) -> Self {
        CaptureStreamRequest::at(value.tag, value.present_seq)
    }
}

impl From<CaptureStreamFault> for AmmCaptureStreamFault {
    fn from(value: CaptureStreamFault) -> Self {
        match value {
            CaptureStreamFault::CopyFailed => AmmCaptureStreamFault::CopyFailed,
            CaptureStreamFault::UnsupportedFormat { .. } => AmmCaptureStreamFault::UnsupportedFormat,
            CaptureStreamFault::TimedOut => AmmCaptureStreamFault::TimedOut,
            CaptureStreamFault::DeviceLost => AmmCaptureStreamFault::DeviceLost,
            CaptureStreamFault::NotPermitted => AmmCaptureStreamFault::NotPermitted,
            CaptureStreamFault::Resized => AmmCaptureStreamFault::Resized,
            CaptureStreamFault::Missed => AmmCaptureStreamFault::Missed,
        }
    }
}

/// A good frame: its header, and its pixels lent for as long as the frame's buffer does not change.
impl From<&CaptureStreamFrame> for AmmCaptureStreamFrame {
    fn from(value: &CaptureStreamFrame) -> Self {
        Self {
            tag: value.tag,
            width: value.width,
            height: value.height,
            row_pitch: value.row_pitch,
            format: value.format,
            fault: AmmCaptureStreamFault::None as u8,
            pixels: value.pixels.as_ptr(),
            pixels_len: value.pixels.len(),
            present_seq: value.present_seq,
            ..Self::blank()
        }
    }
}

/// A lent frame: its header and lease, and its pixels, valid until the lease is given back.
impl From<&LentStreamFrame> for AmmCaptureStreamFrame {
    fn from(value: &LentStreamFrame) -> Self {
        Self {
            tag: value.tag,
            width: value.width,
            height: value.height,
            row_pitch: value.row_pitch,
            format: value.format,
            fault: AmmCaptureStreamFault::None as u8,
            pixels: value.pixels.as_ptr(),
            pixels_len: value.pixels.len(),
            present_seq: value.present_seq,
            lend_id: value.lend_id,
            ..Self::blank()
        }
    }
}

/// A failed frame: its tag and fault, no pixels, and the format an unsupported format names.
impl From<CaptureStreamFailure> for AmmCaptureStreamFrame {
    fn from(value: CaptureStreamFailure) -> Self {
        let format = match value.fault {
            CaptureStreamFault::UnsupportedFormat { format } => format,
            _ => 0,
        };
        Self { tag: value.tag, format, fault: AmmCaptureStreamFault::from(value.fault) as u8, ..Self::blank() }
    }
}

impl AmmCaptureStreamFrame {
    /// The failure this frame reports, or `None` for a good frame. A fault byte this client does not know, from a
    /// newer manager, reads as `CopyFailed`.
    pub fn failure(&self) -> Option<CaptureStreamFailure> {
        let fault = match self.fault {
            0 => return None,
            2 => CaptureStreamFault::UnsupportedFormat { format: self.format },
            3 => CaptureStreamFault::TimedOut,
            4 => CaptureStreamFault::DeviceLost,
            5 => CaptureStreamFault::NotPermitted,
            6 => CaptureStreamFault::Resized,
            7 => CaptureStreamFault::Missed,
            _ => CaptureStreamFault::CopyFailed,
        };
        Some(CaptureStreamFailure::new(self.tag, fault))
    }
}
