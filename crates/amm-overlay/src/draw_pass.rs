use amm_ffi::types::{AMM_HOST_MAX_CAPTURES, AMM_HOST_MAX_STREAM_FRAMES, AmmHostFrame};
use windows::Win32::Graphics::Direct3D11::{ID3D11RenderTargetView, ID3D11Resource};
use windows::Win32::Graphics::Direct3D11on12::ID3D11On12Device;

use crate::capture::FrameCapture;
use crate::device::OverlayDevice;
use crate::host_capture_sink::HostCaptureSink;
use crate::host_stream_sink::HostStreamSink;
use crate::layer_renderer::LayerRenderer;
use crate::state_backup::StateBackup;
use crate::wrapped_acquire::WrappedAcquire;

/// Draws one host frame on one back buffer, inside one Present. The game picture and the game-only captures are
/// copied first, then the capture stream frames, which are always game-only. Then the mod layers (every layer
/// before the first manager layer) are drawn, the layer captures copied, and the manager layer drawn; with no layer
/// capture to take between them, all layers are drawn in one range. The target is unbound while the back buffer is
/// copied, so a copy never reads a bound output.
pub(crate) struct DrawPass<'a> {
    pub(crate) device: &'a OverlayDevice,
    pub(crate) renderer: &'a mut LayerRenderer,
    pub(crate) capture: &'a mut FrameCapture<AMM_HOST_MAX_CAPTURES>,
    pub(crate) sink: HostCaptureSink<'a>,
    pub(crate) stream: &'a mut FrameCapture<AMM_HOST_MAX_STREAM_FRAMES>,
    pub(crate) stream_sink: HostStreamSink<'a>,
    pub(crate) size: (u32, u32),
}

impl DrawPass<'_> {
    /// The D3D11 path: the context is the game's own, so its state is put back after the draw. `CopyResource` binds
    /// nothing, so the copies before the layers leave it as it was.
    ///
    /// # Safety
    ///
    /// `frame` is as `LayerRenderer::draw` requires.
    pub(crate) unsafe fn native(&mut self, target: &ID3D11RenderTargetView, frame: &AmmHostFrame) {
        // SAFETY: the caller vouches for the frame
        let mod_end = unsafe { LayerRenderer::mod_layer_end(frame) };
        let back_buffer =
            if frame.game_picture || frame.capture_count > 0 || frame.layer_capture_count > 0 || frame.stream_count > 0
            {
                // SAFETY: the view is live
                unsafe { target.GetResource() }.ok()
            } else {
                None
            };
        self.copy_before_layers(back_buffer.as_ref(), frame, mod_end);
        if frame.layer_count == 0 {
            return;
        }
        let _state = StateBackup::capture(&self.device.ctx);
        // SAFETY: as above
        unsafe { self.draw_layers(target, back_buffer.as_ref(), frame, mod_end) };
    }

    /// The D3D12 path: the copies and the draw run on the acquired wrapper of the back buffer, which is unbound and
    /// released after, also when a panic unwinds through the draw, and the context is flushed so the queue gets the
    /// work in this Present.
    ///
    /// # Safety
    ///
    /// As for `native`.
    pub(crate) unsafe fn wrapped(
        &mut self,
        on12: &ID3D11On12Device,
        wrapped: &ID3D11Resource,
        target: &ID3D11RenderTargetView,
        frame: &AmmHostFrame,
    ) {
        // SAFETY: the caller vouches for the frame
        let mod_end = unsafe { LayerRenderer::mod_layer_end(frame) };
        let device = self.device;
        let _acquired = WrappedAcquire::new(on12, &device.ctx, wrapped);
        self.copy_before_layers(Some(wrapped), frame, mod_end);
        if frame.layer_count > 0 {
            // SAFETY: as above
            unsafe { self.draw_layers(target, Some(wrapped), frame, mod_end) };
        }
    }

    fn copy_before_layers(&mut self, back_buffer: Option<&ID3D11Resource>, frame: &AmmHostFrame, mod_end: usize) {
        let Some(back_buffer) = back_buffer else {
            return;
        };
        if frame.game_picture {
            self.renderer.game_picture_mut().update(&self.device.device, &self.device.ctx, back_buffer);
        }
        self.copy_captures(back_buffer, &frame.captures, frame.capture_count);
        self.copy_stream(back_buffer, frame);
        if mod_end == 0 {
            self.copy_captures(back_buffer, &frame.layer_captures, frame.layer_capture_count);
        }
    }

    unsafe fn draw_layers(
        &mut self,
        target: &ID3D11RenderTargetView,
        back_buffer: Option<&ID3D11Resource>,
        frame: &AmmHostFrame,
        mod_end: usize,
    ) {
        let ctx = &self.device.ctx;
        let count = frame.layer_count as usize;
        if mod_end == 0 || frame.layer_capture_count == 0 {
            // SAFETY: the caller vouches for the frame
            unsafe { self.renderer.draw(ctx, target, self.size, frame, 0..count) };
            return;
        }
        // SAFETY: as above
        unsafe { self.renderer.draw(ctx, target, self.size, frame, 0..mod_end) };
        self.unbind();
        if let Some(back_buffer) = back_buffer {
            self.copy_captures(back_buffer, &frame.layer_captures, frame.layer_capture_count);
        }
        // SAFETY: as above
        unsafe { self.renderer.draw(&self.device.ctx, target, self.size, frame, mod_end..count) };
    }

    fn copy_captures(&mut self, back_buffer: &ID3D11Resource, sequences: &[u32; AMM_HOST_MAX_CAPTURES], count: u32) {
        if count == 0 {
            return;
        }
        let sequences = &sequences[..(count as usize).min(AMM_HOST_MAX_CAPTURES)];
        self.capture.request(&self.device.device, &self.device.ctx, back_buffer, sequences, &mut self.sink);
    }

    fn copy_stream(&mut self, back_buffer: &ID3D11Resource, frame: &AmmHostFrame) {
        if frame.stream_count == 0 {
            return;
        }
        let ids = &frame.streams[..(frame.stream_count as usize).min(AMM_HOST_MAX_STREAM_FRAMES)];
        self.stream.request(&self.device.device, &self.device.ctx, back_buffer, ids, &mut self.stream_sink);
    }

    // `LayerRenderer::draw` leaves its target bound
    fn unbind(&self) {
        // SAFETY: binding no target takes no object
        unsafe { self.device.ctx.OMSetRenderTargets(None, None) };
    }
}
