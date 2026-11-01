use std::path::Path;

use amm_core::errors::StoreError;
use amm_core::services::manager::ManagerService;
use amm_core::structs::capture::{CaptureImage, PixelFormat, ScreenshotLayers};
use amm_store::services::IoThread;

use super::CaptureSlot;
use crate::types::{AMM_HOST_CAPTURE_MAX_FRAMES, AMM_HOST_MAX_CAPTURES, AmmHostFrame};

/// The captures between the overlay and the I/O thread. Each Present it offers the captures the manager has
/// pending, ages the ones offered before, and hands each one the overlay reported to the manager: a good copy goes
/// to the I/O thread as a PNG job, a failed copy, an unsupported format, a buffer not reported in the Present
/// that asked for it, or a capture not reported within `MAX_FRAMES` Presents ends with
/// `ScreenshotTaken { ok: false }`. Bounds: `AMM_HOST_MAX_CAPTURES` slots, the
/// same count as the manager's capture table, and one pixel buffer per slot, made when the overlay asks for it.
/// A Present with no capture in flight does not allocate.
pub struct CaptureRelay {
    slots: [Option<CaptureSlot>; AMM_HOST_MAX_CAPTURES],
}

impl CaptureRelay {
    /// The Presents a capture may take from its offer to the overlay's report.
    const MAX_FRAMES: u32 = AMM_HOST_CAPTURE_MAX_FRAMES;
    /// The largest width or height of a capture, the largest D3D11 texture side.
    const MAX_SIDE: u32 = 16_384;

    pub fn new() -> Self {
        Self { slots: std::array::from_fn(|_| None) }
    }

    /// True while a capture waits for the overlay or for its hand-off.
    pub fn is_active(&self) -> bool {
        self.slots.iter().any(Option::is_some)
    }

    /// Hands on the captures the overlay reported since the last Present and fails the ones too old. Runs once per
    /// Present, before `offer`.
    pub fn advance(&mut self, manager: &mut ManagerService, io: Option<&IoThread>) {
        for slot in self.slots.iter_mut() {
            let Some(capture) = slot.as_mut() else {
                continue;
            };
            if capture.report.is_none() {
                capture.age += 1;
                if capture.has_buffer() {
                    curia::warn!("screenshot buffer not reported in the Present that asked for it", {
                        sequence: capture.sequence,
                    });
                } else if capture.age < Self::MAX_FRAMES {
                    continue;
                } else {
                    curia::warn!("screenshot not copied in time", { sequence: capture.sequence, frames: capture.age });
                }
            }
            if let Some(capture) = slot.take() {
                Self::hand_on(capture, manager, io);
            }
        }
    }

    /// Takes the captures the manager has pending for this Present. Returns an empty frame that carries only
    /// their sequences: the game-only ones in `captures`, the ones with mod layers in `layer_captures`. Both sets
    /// share the slots. A capture with no free slot fails at once.
    pub fn offer(&mut self, manager: &mut ManagerService) -> AmmHostFrame {
        let mut frame = AmmHostFrame::empty();
        let (mut game_count, mut layer_count) = (0, 0);
        for request in manager.take_captures().as_slice() {
            let (set, count) = match request.layers {
                ScreenshotLayers::GameOnly => (&mut frame.captures, &mut game_count),
                ScreenshotLayers::WithModLayers => (&mut frame.layer_captures, &mut layer_count),
            };
            let free = self.slots.iter_mut().find(|slot| slot.is_none());
            match (free, set.get_mut(*count)) {
                (Some(free), Some(out)) => {
                    *free = Some(CaptureSlot::new(request.sequence));
                    *out = request.sequence;
                    *count += 1;
                }
                _ => {
                    manager.capture_taken(request.sequence, false);
                }
            }
        }
        frame.capture_count = u32::try_from(game_count).unwrap_or(0);
        frame.layer_capture_count = u32::try_from(layer_count).unwrap_or(0);
        frame
    }

    /// The buffer the overlay copies the pixels of `sequence` into, and its row pitch: `height` rows of
    /// `width * 4` bytes, valid until the next `advance` or `device_lost`. The first call reserves it, not
    /// cleared; the overlay must write every row in full before it reports the capture done with ok 1. A later
    /// call with the same size returns the same buffer. `None` for a sequence not offered or already reported, a side
    /// of 0 or past `MAX_SIDE`, or another size. A buffer that cannot be allocated gives `None` and fails the
    /// capture at the next `advance`.
    pub fn buffer(&mut self, sequence: u32, width: u32, height: u32) -> Option<(*mut u8, u32)> {
        let valid = (1..=Self::MAX_SIDE).contains(&width) && (1..=Self::MAX_SIDE).contains(&height);
        let capture = self.waiting(sequence).filter(|_| valid)?;
        if !capture.has_buffer() {
            let row_pitch = width.checked_mul(PixelFormat::BYTES_PER_PIXEL)?;
            let len = usize::try_from(row_pitch).ok()?.checked_mul(usize::try_from(height).ok()?)?;
            if capture.pixels.try_reserve_exact(len).is_err() {
                curia::warn!("screenshot buffer not allocated", { sequence: sequence, bytes: len });
                capture.report = Some((false, 0));
                return None;
            }
            (capture.width, capture.height, capture.row_pitch) = (width, height, row_pitch);
        }
        if (capture.width, capture.height) != (width, height) {
            return None;
        }
        Some((capture.pixels.spare_capacity_mut().as_mut_ptr().cast::<u8>(), capture.row_pitch))
    }

    /// Keeps the overlay's report for `sequence`, handed on at the next Present. A good report of a capture with a
    /// buffer makes its pixels part of the buffer. False for a sequence not offered or already reported.
    pub fn report(&mut self, sequence: u32, ok: bool, format: u32) -> bool {
        let Some(capture) = self.waiting(sequence) else {
            return false;
        };
        if ok && capture.has_buffer() {
            let len = capture.buffer_len();
            // SAFETY: the buffer reserved exactly `len` bytes for `height` rows of `width * 4`, and the overlay writes
            // every byte of every row before it reports ok 1: its copyRows fails the capture, with ok 0, when the
            // mapped row pitch or this row pitch is shorter than `width * 4`
            unsafe { capture.pixels.set_len(len) };
        }
        capture.report = Some((ok, format));
        true
    }

    /// The device dropped its resources, the staging textures with them: every capture not reported yet fails.
    pub fn device_lost(&mut self) {
        for capture in self.slots.iter_mut().flatten().filter(|capture| capture.report.is_none()) {
            capture.report = Some((false, 0));
        }
    }

    /// Gives the manager the outcome of each capture the I/O thread finished. It does not allocate.
    pub fn collect_written(manager: &mut ManagerService, io: Option<&IoThread>) {
        let Some(io) = io else {
            return;
        };
        for outcome in io.written_captures() {
            manager.capture_written(outcome.sequence, outcome.ok);
        }
    }

    fn waiting(&mut self, sequence: u32) -> Option<&mut CaptureSlot> {
        self.slots.iter_mut().flatten().find(|capture| capture.sequence == sequence && capture.report.is_none())
    }

    // Ends the copy step of one capture in the manager and, when the manager still wants it, queues its write.
    fn hand_on(capture: CaptureSlot, manager: &mut ManagerService, io: Option<&IoThread>) {
        let (copied, format) = capture.report.unwrap_or((false, 0));
        let readable = PixelFormat::from_dxgi(format).is_some();
        if copied && capture.pixels.is_empty() {
            curia::warn!("screenshot reported copied with no buffer asked for", { sequence: capture.sequence });
        } else if copied && !readable {
            curia::warn!("screenshot refused, the back buffer format is not supported", {
                sequence: capture.sequence,
                format: format,
            });
        }
        let ok = copied && readable && !capture.pixels.is_empty();
        if !manager.capture_taken(capture.sequence, ok) {
            return;
        }
        let sequence = capture.sequence;
        let Some(path) = manager.capture_path(sequence).map(Path::to_path_buf) else {
            manager.capture_written(sequence, false);
            return;
        };
        let image = CaptureImage {
            sequence,
            path,
            pixels: capture.pixels,
            width: capture.width,
            height: capture.height,
            row_pitch: capture.row_pitch,
            format,
        };
        let queued = io.map_or(Err(StoreError::Unavailable), |io| io.submit_capture(image));
        if let Err(error) = queued {
            curia::warn!("screenshot not queued for writing", { sequence: sequence, error: error.to_string() });
            manager.capture_written(sequence, false);
        }
    }
}

impl Default for CaptureRelay {
    fn default() -> Self {
        Self::new()
    }
}
