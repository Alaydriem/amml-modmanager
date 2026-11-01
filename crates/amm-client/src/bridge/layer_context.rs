use std::sync::Arc;

use amm_abi::types::{AmmCursorShape, AmmFrameOutput, AmmMeshView};
use amm_core::services::egui_bridge::{MeshFlattener, RawInputBuilder};
use amm_core::structs::frame::LayerKind;
use amm_core::structs::mesh::MeshBuffers;
use amml_i18n::localize::Localizer;
use amml_ui::font::FontSource;
use amml_ui::text::UiLocale;
use amml_ui::theme::Theme;
use egui::{Context, Event, OutputCommand, PlatformOutput, RawInput};

use super::ClipboardWorker;
use crate::link::{FrameHandler, FrameInputView, FrameReply};

/// The egui runtime of one layer of a mod: its context, raw-input builder and the mesh buffers the host reads.
/// The buffers keep their capacity across frames. Copy and paste go through the mod's `ClipboardWorker`: a
/// Ctrl+V asks for the clipboard text, and a later pass of this layer gets it as `Event::Paste`. Managed texture
/// ids leave with the layer bit of `MeshFlattener`, so the ids of the Hud and Overlay contexts never meet in the
/// host.
pub struct LayerContext {
    layer: LayerKind,
    ctx: Context,
    // the localizer this context holds now; none on a new context, so the next pass sets it
    localizer: Option<Arc<Localizer>>,
    builder: RawInputBuilder,
    mesh: MeshBuffers,
    // a text field of this layer had keyboard focus at the end of the last pass
    text_focused: bool,
    // a new context the handler has not been told about yet
    fresh: bool,
}

impl LayerContext {
    const FONTS: FontSource = FontSource::Embedded;

    pub fn new(layer: LayerKind) -> Self {
        Self {
            layer,
            ctx: Self::context(),
            localizer: None,
            builder: RawInputBuilder::new(),
            mesh: MeshBuffers::new(),
            text_focused: false,
            fresh: true,
        }
    }

    /// Runs one pass of `handler` for this layer and fills `out`. `out.mesh` points at this layer's own buffers,
    /// in physical pixels, and stays valid until the next `run` of this layer.
    pub fn run<H: FrameHandler>(
        &mut self,
        handler: &mut H,
        input: &FrameInputView,
        clipboard: &ClipboardWorker,
        out: &mut AmmFrameOutput,
    ) {
        let mut raw = RawInput::default();
        self.builder.build(input.data(), &mut raw);
        self.paste(clipboard, &mut raw);
        handler.adjust_input(self.layer, &mut raw);
        let mut reply = None;
        let mut output = self.ctx.run_pass(raw, |ctx| reply = Some(handler.frame(ctx, input)));
        let ppp = output.pixels_per_point;
        // True while the focused widget is a text edit, also when it is scrolled out of view; false for a button
        let text_focused = self.ctx.text_edit_focused();
        self.text_focused = text_focused;
        Self::copy(clipboard, &mut output.platform_output);
        let primitives = self.ctx.tessellate(std::mem::take(&mut output.shapes), ppp);
        let textures = self.ctx.tex_manager();
        MeshFlattener::flatten_layer(
            &primitives,
            &output.textures_delta,
            &textures.read(),
            ppp,
            self.layer,
            &mut self.mesh,
        );
        output.textures_delta.clear();
        self.fill(out, reply, ppp, text_focused);
    }

    /// Replaces the context, so the next pass sends the font atlas again in full under the same ids. Used when the
    /// host lost its textures and after a panic left the context mid-pass.
    pub fn reset(&mut self) {
        self.ctx = Self::context();
        self.localizer = None;
        self.text_focused = false;
        self.fresh = true;
    }

    /// True once for each new context: the first time it is asked after `new` or `reset`.
    pub fn take_fresh(&mut self) -> bool {
        std::mem::replace(&mut self.fresh, false)
    }

    /// The layer's egui context.
    pub fn context_ref(&self) -> &Context {
        &self.ctx
    }

    /// The localizer the layer's widgets resolve their text with in the next pass. It is put into the context only
    /// when it is not the one the context holds.
    pub fn use_localizer(&mut self, localizer: Arc<Localizer>) {
        if self.localizer.as_ref().is_some_and(|held| Arc::ptr_eq(held, &localizer)) {
            return;
        }
        UiLocale::set(&self.ctx, Arc::clone(&localizer));
        self.localizer = Some(localizer);
    }

    // Gives a pasted text that came back to the focused field, and asks for one on Ctrl+V. Both need a field
    // that had focus in the last pass; a text that comes back after the field lost focus is dropped.
    fn paste(&mut self, clipboard: &ClipboardWorker, raw: &mut RawInput) {
        let pasted = clipboard.take_paste(self.layer);
        let asked = self.builder.take_paste_request();
        if !self.text_focused {
            return;
        }
        if let Some(text) = pasted {
            raw.events.push(Event::Paste(text));
        }
        if asked {
            clipboard.request_paste(self.layer);
        }
    }

    fn copy(clipboard: &ClipboardWorker, output: &mut PlatformOutput) {
        for command in output.commands.drain(..) {
            if let OutputCommand::CopyText(text) = command {
                clipboard.copy(text);
            }
        }
    }

    fn context() -> Context {
        let ctx = Context::default();
        Theme::install(&ctx, Self::FONTS);
        ctx
    }

    fn fill(&self, out: &mut AmmFrameOutput, reply: Option<FrameReply>, ppp: f32, text_focused: bool) {
        out.mesh = AmmMeshView::from(&self.mesh);
        out.wants_pointer = u8::from(self.ctx.egui_wants_pointer_input());
        out.wants_keyboard = u8::from(text_focused);
        let (cursor, rect) = match reply {
            Some(reply) => (AmmCursorShape::from(reply.cursor), reply.layer_rect),
            None => (AmmCursorShape::Default, egui::Rect::NOTHING),
        };
        out.cursor = cursor as u8;
        // An empty or inverted rect, such as Rect::NOTHING, reaches the host as an empty rect at the origin
        out.layer_rect = if rect.is_positive() {
            [rect.min.x * ppp, rect.min.y * ppp, rect.max.x * ppp, rect.max.y * ppp]
        } else {
            [0.0; 4]
        };
    }
}
