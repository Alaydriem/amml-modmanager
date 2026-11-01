#![allow(unsafe_code)]

use std::time::{Duration, Instant};

use amm_abi::types::{AmmHandle, AmmStatus};
use amm_core::structs::capture::CaptureStreamRequest;
use amm_core::structs::descriptor::Capability;
use amm_ffi::host::capture_stream::{amm_host_stream_buffer, amm_host_stream_done};
use amm_ffi::services::HostCell;

use super::{BenchHost, Descriptors, FakeMesh};

/// The host with one mod on a Hud lease that asks for one capture stream frame per Present, for timing what the
/// render thread does with one 4K stream frame inside Present: ask for the buffer, copy the rows into it (about
/// 33 MB), report it done. The mod takes each frame outside the timed part, so the ring never fills.
pub struct StreamRig {
    host: BenchHost,
    handle: AmmHandle,
    source: Vec<u8>,
    now_ms: u64,
    taken: usize,
}

impl StreamRig {
    pub const WIDTH: u32 = 3840;
    pub const HEIGHT: u32 = 2160;
    const BYTES_PER_PIXEL: usize = 4;
    const DXGI_R8G8B8A8_UNORM: u32 = 28;
    const PATTERN: u8 = 0x5a;
    const FRAME_STEP_MS: u64 = 16;

    pub fn new() -> Self {
        let mut host = BenchHost::new();
        let descriptor = Descriptors::with_claims("renderer", &[("Hud", "Shared")]);
        let handle = host.add_client(&descriptor, FakeMesh::empty());
        host.acquire(handle, Capability::Hud);
        let len = Self::WIDTH as usize * Self::BYTES_PER_PIXEL * Self::HEIGHT as usize;
        Self { host, handle, source: vec![Self::PATTERN; len], now_ms: 0, taken: 0 }
    }

    /// Requests a frame and runs the Present that offers it, then times `amm_host_stream_buffer`, the copy of
    /// every row and `amm_host_stream_done`. The mod then takes the frame outside the timed part.
    pub fn take_one(&mut self) -> Duration {
        let tag = self.taken as u64;
        let requested = HostCell::global()
            .with(|runtime| runtime.request_stream_frame(self.handle, CaptureStreamRequest::new(tag)));
        requested.unwrap_or_else(|status| panic!("request_stream_frame: {status:?}"));
        self.now_ms += Self::FRAME_STEP_MS;
        let frame = self.host.frame_at(self.now_ms, Self::WIDTH, Self::HEIGHT);
        assert_eq!(frame.stream_count, 1);
        let id = frame.streams[0];
        let row = Self::WIDTH as usize * Self::BYTES_PER_PIXEL;

        let start = Instant::now();
        let mut pitch = 0;
        // SAFETY: pitch lives for the call
        let buffer = unsafe { amm_host_stream_buffer(id, Self::WIDTH, Self::HEIGHT, &mut pitch) };
        assert!(!buffer.is_null(), "no stream buffer");
        for (y, source) in self.source.chunks_exact(row).enumerate() {
            // SAFETY: the host gives HEIGHT rows of pitch bytes at buffer, valid until the next host frame, and each
            // row of WIDTH * 4 bytes fits in its pitch
            unsafe { std::ptr::copy_nonoverlapping(source.as_ptr(), buffer.add(y * pitch as usize), row) };
        }
        let reported = amm_host_stream_done(id, 1, Self::DXGI_R8G8B8A8_UNORM);
        let elapsed = start.elapsed();

        assert_eq!(reported, AmmStatus::Ok);
        let taken = HostCell::global().with(|runtime| runtime.take_stream_frame(self.handle));
        let taken = taken.unwrap_or_else(|status| panic!("take_stream_frame: {status:?}"));
        assert_eq!((taken.tag, taken.width, taken.height, taken.fault), (tag, Self::WIDTH, Self::HEIGHT, 0));
        self.taken += 1;
        elapsed
    }

    /// The frames taken so far.
    pub fn taken(&self) -> usize {
        self.taken
    }
}

impl Default for StreamRig {
    fn default() -> Self {
        Self::new()
    }
}
