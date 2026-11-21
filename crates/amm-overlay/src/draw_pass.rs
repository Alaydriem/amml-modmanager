use std::ops::Range;

use amm_ffi::types::AmmHostFrame;
use windows::Win32::Graphics::Direct3D11::{ID3D11DeviceContext, ID3D11RenderTargetView};

use crate::draw_report::DrawReport;
use crate::layer_renderer::LayerRenderer;

/// Draws a range of the layers of one host frame on one target, inside one `FramePresent` call, bottom to top. The
/// target is unbound after the draw, so the context holds no reference to the back buffer past the call.
pub(crate) struct DrawPass<'a> {
    pub(crate) renderer: &'a mut LayerRenderer,
    pub(crate) ctx: &'a ID3D11DeviceContext,
    pub(crate) target: &'a ID3D11RenderTargetView,
    pub(crate) size: (u32, u32),
}

impl DrawPass<'_> {
    /// Runs the once-per-frame part of `frame`, then draws `layers` of it. An empty range costs no D3D call.
    ///
    /// # Safety
    ///
    /// `frame` is as `LayerRenderer::draw` requires.
    pub(crate) unsafe fn first(self, frame: &AmmHostFrame, layers: Range<usize>) -> DrawReport {
        // SAFETY: the caller vouches for the frame
        unsafe { self.renderer.begin_frame(frame) };
        // SAFETY: as above
        unsafe { self.rest(frame, layers) }
    }

    /// Draws `layers` of a frame whose `first` pass already ran. An empty range costs no D3D call.
    ///
    /// # Safety
    ///
    /// `frame` is as `LayerRenderer::draw` requires.
    pub(crate) unsafe fn rest(self, frame: &AmmHostFrame, layers: Range<usize>) -> DrawReport {
        if layers.is_empty() {
            return DrawReport::default();
        }
        // SAFETY: the caller vouches for the frame
        let report = unsafe { self.renderer.draw(self.ctx, self.target, self.size, frame, layers) };
        // SAFETY: binding no target takes no object
        unsafe { self.ctx.OMSetRenderTargets(None, None) };
        report
    }
}
