#![allow(unsafe_code)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use amm_core::config::ManagerConfig;
use amm_core::services::ManagerService;
use amm_core::structs::capture::ScreenshotLayers;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::registry::ModIndex;
use amm_ffi::services::CaptureRelay;

use super::Descriptors;

/// A manager with one mod on a Hud lease and the host's capture relay, for timing what the render thread does
/// with one 4K capture inside Present: ask for the buffer, copy the rows into it, report it done. There is no
/// I/O thread, so each capture ends without a file at the next `advance`.
pub struct CaptureRig {
    manager: ManagerService,
    relay: CaptureRelay,
    index: ModIndex,
    path: PathBuf,
    source: Vec<u8>,
}

impl CaptureRig {
    pub const WIDTH: u32 = 3840;
    pub const HEIGHT: u32 = 2160;
    const BYTES_PER_PIXEL: usize = 4;
    const DXGI_R8G8B8A8_UNORM: u32 = 28;
    const PATTERN: u8 = 0x5a;

    pub fn new() -> Self {
        let mut manager = ManagerService::new(ManagerConfig::default(), None);
        let descriptor = Descriptors::with_claims("shooter", &[("Hud", "Shared")]);
        let (index, _) = manager
            .register(&descriptor, None, Path::new("C:\\bench\\shooter"))
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        manager.acquire(index, Capability::Hud, 0).unwrap_or_else(|error| panic!("acquire: {error:?}"));
        let len = Self::WIDTH as usize * Self::BYTES_PER_PIXEL * Self::HEIGHT as usize;
        Self {
            manager,
            relay: CaptureRelay::new(),
            index,
            path: PathBuf::from("C:\\bench\\shots\\shot.png"),
            source: vec![Self::PATTERN; len],
        }
    }

    /// Books and offers one capture, then times `buffer`, the copy of every row and `report`. The capture is then
    /// ended outside the timed part.
    pub fn take_one(&mut self) -> Duration {
        let sequence = self
            .manager
            .request_capture(self.index, &self.path, ScreenshotLayers::GameOnly)
            .unwrap_or_else(|error| panic!("request_capture: {error:?}"));
        let offered = self.relay.offer(&mut self.manager);
        assert_eq!((offered.captures[0], offered.capture_count), (sequence, 1));
        let row = Self::WIDTH as usize * Self::BYTES_PER_PIXEL;

        let start = Instant::now();
        let (buffer, pitch) =
            self.relay.buffer(sequence, Self::WIDTH, Self::HEIGHT).unwrap_or_else(|| panic!("no capture buffer"));
        for (y, source) in self.source.chunks_exact(row).enumerate() {
            // SAFETY: the relay gives HEIGHT rows of pitch bytes at buffer, valid until the next advance, and each
            // row of WIDTH * 4 bytes fits in its pitch
            unsafe { std::ptr::copy_nonoverlapping(source.as_ptr(), buffer.add(y * pitch as usize), row) };
        }
        let reported = self.relay.report(sequence, true, Self::DXGI_R8G8B8A8_UNORM);
        let elapsed = start.elapsed();

        assert!(reported);
        self.relay.advance(&mut self.manager, None);
        elapsed
    }
}

impl Default for CaptureRig {
    fn default() -> Self {
        Self::new()
    }
}
