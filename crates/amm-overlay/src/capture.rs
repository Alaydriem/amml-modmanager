use amm_ffi::types::AMM_HOST_CAPTURE_MAX_FRAMES;
use windows::Win32::Foundation::{E_NOINTERFACE, E_NOTIMPL, E_OUTOFMEMORY, S_FALSE};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_MAP_FLAG_DO_NOT_WAIT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_STAGING, ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::DXGI_ERROR_WAS_STILL_DRAWING;
use windows::core::{HRESULT, Interface};

use crate::capture_kind::CaptureKind;
use crate::capture_sink::CaptureSink;
use crate::overlay_shared::OverlayShared;
use crate::pace_clock::PaceClock;
use crate::staging_slot::StagingSlot;

/// Copies the back buffer for the screenshots or the capture stream frames the manager asks for and hands the
/// pixels to a `CaptureSink`, with no wait on the GPU. Render thread only, inside Present. A fixed pool of `SLOTS`
/// staging textures, one pool per `CaptureKind`: `request` issues one `CopyResource` per sequence it is given, and
/// `poll` tries a non-blocking `Map` of each busy texture on each later Present. A mapped copy goes into the sink's
/// buffer and is reported done in the same call. A copy that cannot be made, mapped within `MAP_WAIT_FRAMES`
/// Presents, or written is reported with `ok` false. A multisampled back buffer is not resolved: its captures are
/// reported with `ok` false.
pub struct FrameCapture<const SLOTS: usize> {
    kind: CaptureKind,
    slots: [StagingSlot; SLOTS],
    // a multisampled back buffer was logged once this session
    multisample_logged: bool,
}

impl<const SLOTS: usize> FrameCapture<SLOTS> {
    pub const MAP_WAIT_FRAMES: u32 = AMM_HOST_CAPTURE_MAX_FRAMES;
    const BYTES_PER_PIXEL: usize = 4;

    pub fn new(kind: CaptureKind) -> Self {
        Self { kind, slots: std::array::from_fn(|_| StagingSlot::new()), multisample_logged: false }
    }

    /// Copies `back_buffer` into a free staging texture for each of the first `SLOTS` of
    /// `sequences`: the game-only set of a frame or its layer set, each at its own capture point. On D3D11On12
    /// call it while the wrapped back buffer is acquired.
    pub fn request(
        &mut self,
        device: &ID3D11Device,
        ctx: &ID3D11DeviceContext,
        back_buffer: &ID3D11Resource,
        sequences: &[u32],
        sink: &mut impl CaptureSink,
    ) {
        let sequences = &sequences[..sequences.len().min(SLOTS)];
        let texture = back_buffer.cast::<ID3D11Texture2D>().ok();
        for &sequence in sequences {
            match &texture {
                Some(texture) => self.copy_one(device, ctx, back_buffer, texture, sequence, sink),
                None => Self::fail(
                    self.kind,
                    sink,
                    sequence,
                    "the back buffer is not a readable 2D texture",
                    E_NOINTERFACE,
                    DXGI_FORMAT_UNKNOWN,
                ),
            }
        }
    }

    /// Tries to read each busy staging texture without waiting, and reports each capture that is read or failed.
    pub fn poll(&mut self, ctx: &ID3D11DeviceContext, sink: &mut impl CaptureSink) {
        let kind = self.kind;
        for slot in self.slots.iter_mut() {
            if slot.busy {
                Self::poll_one(kind, ctx, slot, sink);
            }
        }
    }

    /// True while a copy waits to be read.
    pub fn in_flight(&self) -> bool {
        self.slots.iter().any(|slot| slot.busy)
    }

    /// Drops every staging texture, for a resize, a device loss or shutdown. True when a capture was in flight:
    /// the caller then tells the manager its textures are lost, which fails it. The stream pool is also released at
    /// the end of the stream, where the caller tells the manager nothing.
    pub fn release(&mut self) -> bool {
        let in_flight = self.in_flight();
        for slot in self.slots.iter_mut() {
            slot.reset();
        }
        in_flight
    }

    fn copy_one(
        &mut self,
        device: &ID3D11Device,
        ctx: &ID3D11DeviceContext,
        back_buffer: &ID3D11Resource,
        texture: &ID3D11Texture2D,
        sequence: u32,
        sink: &mut impl CaptureSink,
    ) {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: `texture` is live and `desc` is writable for the call
        unsafe { texture.GetDesc(&mut desc) };
        if desc.SampleDesc.Count > 1 {
            if !self.multisample_logged {
                Self::log_failure(self.kind, sequence, "the back buffer is multisampled", E_NOTIMPL);
            }
            self.multisample_logged = true;
            sink.done(sequence, false, desc.Format);
            return;
        }
        let Some(slot) = self.slots.iter_mut().find(|slot| !slot.busy) else {
            Self::fail(self.kind, sink, sequence, "no free staging texture", E_OUTOFMEMORY, desc.Format);
            return;
        };
        if let Err(result) = Self::ensure_texture(device, slot, &desc) {
            Self::fail(self.kind, sink, sequence, "the staging texture could not be made", result, desc.Format);
            return;
        }
        if let Some(staging) = &slot.texture {
            // SAFETY: both resources are live and the staging texture has the back buffer's size and format
            unsafe { ctx.CopyResource(staging, back_buffer) };
        }
        slot.busy = true;
        slot.sequence = sequence;
        slot.waited = 0;
    }

    // the staging texture of `slot` matches `source`, made again when the size or format changed
    fn ensure_texture(
        device: &ID3D11Device,
        slot: &mut StagingSlot,
        source: &D3D11_TEXTURE2D_DESC,
    ) -> Result<(), HRESULT> {
        if slot.texture.is_some()
            && slot.width == source.Width
            && slot.height == source.Height
            && slot.format == source.Format
        {
            return Ok(());
        }
        slot.texture = None;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: source.Width,
            Height: source.Height,
            MipLevels: 1,
            ArraySize: 1,
            Format: source.Format,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut texture: Option<ID3D11Texture2D> = None;
        // SAFETY: desc lives for the call and the texture has no initial data
        unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }.map_err(|error| error.code())?;
        slot.texture = Some(texture.ok_or(E_OUTOFMEMORY)?);
        slot.width = source.Width;
        slot.height = source.Height;
        slot.format = source.Format;
        Ok(())
    }

    fn poll_one(kind: CaptureKind, ctx: &ID3D11DeviceContext, slot: &mut StagingSlot, sink: &mut impl CaptureSink) {
        let Some(texture) = slot.texture.clone() else {
            slot.free();
            return;
        };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        let started = PaceClock::ticks();
        // SAFETY: the staging texture is live and was made with CPU read access; `mapped` is writable
        let result =
            unsafe { ctx.Map(&texture, 0, D3D11_MAP_READ, D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32, Some(&mut mapped)) };
        if let Err(error) = result {
            let code = error.code();
            if code == DXGI_ERROR_WAS_STILL_DRAWING {
                slot.waited += 1;
                if slot.waited < Self::MAP_WAIT_FRAMES {
                    return;
                }
                Self::fail(kind, sink, slot.sequence, "the copy was not ready in time", code, slot.format);
            } else {
                Self::fail(kind, sink, slot.sequence, "the staging texture could not be mapped", code, slot.format);
            }
            slot.free();
            return;
        }
        let copied = Self::copy_rows(slot, &mapped, sink);
        // the Map and the row copy of a mapped frame: the time the GPU readback costs the render thread
        OverlayShared::global().pace.add_readback(PaceClock::micros_since(started));
        // SAFETY: the texture was mapped above
        unsafe { ctx.Unmap(&texture, 0) };
        if copied {
            sink.done(slot.sequence, true, slot.format);
        } else {
            Self::fail(kind, sink, slot.sequence, "no capture buffer from the manager", S_FALSE, slot.format);
        }
        slot.free();
    }

    // the rows of the mapped copy into the sink's buffer; false when the sink gave no buffer, or a pitch or
    // length too short for every whole row, so a capture is never reported with part of a row left unfilled
    fn copy_rows(slot: &StagingSlot, mapped: &D3D11_MAPPED_SUBRESOURCE, sink: &mut impl CaptureSink) -> bool {
        if mapped.pData.is_null() || slot.height == 0 {
            return false;
        }
        let Some((buffer, row_pitch)) = sink.buffer(slot.sequence, slot.width, slot.height) else {
            return false;
        };
        let row_bytes = slot.width as usize * Self::BYTES_PER_PIXEL;
        let row_pitch = row_pitch as usize;
        let source_pitch = mapped.RowPitch as usize;
        let height = slot.height as usize;
        if source_pitch < row_bytes || row_pitch < row_bytes || buffer.len() < (height - 1) * row_pitch + row_bytes {
            return false;
        }
        // SAFETY: a mapped texture of `height` rows holds `height` rows of `RowPitch` bytes, the last at least
        // `row_bytes` long, for as long as it is mapped
        let source =
            unsafe { std::slice::from_raw_parts(mapped.pData.cast::<u8>(), (height - 1) * source_pitch + row_bytes) };
        for row in 0..height {
            let from = row * source_pitch;
            let to = row * row_pitch;
            buffer[to..to + row_bytes].copy_from_slice(&source[from..from + row_bytes]);
        }
        true
    }

    fn fail(
        kind: CaptureKind,
        sink: &mut impl CaptureSink,
        sequence: u32,
        reason: &'static str,
        result: HRESULT,
        format: DXGI_FORMAT,
    ) {
        Self::log_failure(kind, sequence, reason, result);
        sink.done(sequence, false, format);
    }

    fn log_failure(kind: CaptureKind, sequence: u32, reason: &'static str, result: HRESULT) {
        curia::warn!("frame capture copy failed", {
            kind: kind.name(),
            sequence: sequence,
            reason: reason,
            hresult: result.0 as u32
        });
    }
}
