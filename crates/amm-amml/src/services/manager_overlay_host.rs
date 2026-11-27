use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};

use amm_abi::types::AmmCursorShape;
use amm_core::structs::render::RenderSize;
use amm_ffi::services::HostCell;
use amm_ffi::types::{AmmHostFrame, AmmHostFrameInput};
use amm_overlay::OverlayHost;
use amml_api::Ctx;

/// The manager host as the overlay renderer sees it: each present's host frame (with the screen code of the last
/// tick), the textures-lost signal, and what a frame hands on to the other threads: the cursor shape the layers or
/// the panel ask for, and the size message for the game window. It lives for the process (the renderer needs a
/// `'static` host), inside `ManagerRuntime`; with no host running it gives no frame.
pub struct ManagerOverlayHost {
    cell: &'static HostCell,
    ctx: Ctx,
    // the screen code of the last tick (`TickService::MENU_*`), written on the game thread
    menu: Arc<AtomicU8>,
    // the last host frame; written only by `frame`, which one renderer calls on the render thread, one at a time
    frame: UnsafeCell<AmmHostFrame>,
    // a failed host frame was logged; cleared by the next frame that works
    failure_logged: AtomicBool,
    // the `AmmCursorShape` of the last frame
    cursor: AtomicU8,
    // the size message of the last frame that had one, as `width << 32 | height`; 0 when none waits
    size_message: AtomicU64,
    frames: AtomicU64,
    skipped: AtomicU64,
}

// SAFETY: `frame` is written and read only by `OverlayHost::frame` and the renderer that called it, which runs
// inside one `FramePresent` listener behind the renderer's lock, so never on two threads at once; `ctx` holds the
// loader's tables, which every thread may call; the rest is atomics
#[allow(unsafe_code)]
unsafe impl Sync for ManagerOverlayHost {}

// SAFETY: the frame's pointers point into the host's buffers, not into thread-local memory, and are read only as
// `Sync` above describes; the rest is `Send`
#[allow(unsafe_code)]
unsafe impl Send for ManagerOverlayHost {}

impl ManagerOverlayHost {
    pub fn new(cell: &'static HostCell, ctx: Ctx, menu: Arc<AtomicU8>) -> Self {
        Self {
            cell,
            ctx,
            menu,
            frame: UnsafeCell::new(AmmHostFrame::empty()),
            failure_logged: AtomicBool::new(false),
            cursor: AtomicU8::new(AmmCursorShape::Default as u8),
            size_message: AtomicU64::new(0),
            frames: AtomicU64::new(0),
            skipped: AtomicU64::new(0),
        }
    }

    /// The cursor shape the last frame asked for: `Default` before the first frame and when nothing asks for one.
    pub fn cursor_shape(&self) -> AmmCursorShape {
        match self.cursor.load(Ordering::Relaxed) {
            1 => AmmCursorShape::Arrow,
            2 => AmmCursorShape::Hand,
            3 => AmmCursorShape::Text,
            _ => AmmCursorShape::Default,
        }
    }

    /// The size message the last frames gave for the game window, once; `None` when none waits.
    pub fn take_size_message(&self) -> Option<RenderSize> {
        let packed = self.size_message.swap(0, Ordering::AcqRel);
        (packed != 0).then(|| RenderSize::new((packed >> 32) as u32, packed as u32))
    }

    /// How many host frames the renderer asked for.
    pub fn frames(&self) -> u64 {
        self.frames.load(Ordering::Relaxed)
    }

    /// How many presents drew no frame.
    pub fn skipped(&self) -> u64 {
        self.skipped.load(Ordering::Relaxed)
    }

    // hands on what the other threads read from a frame
    fn publish(&self, frame: &AmmHostFrame) {
        self.cursor.store(frame.cursor as u8, Ordering::Relaxed);
        if frame.size_message_width != 0 && frame.size_message_height != 0 {
            let packed = (u64::from(frame.size_message_width) << 32) | u64::from(frame.size_message_height);
            self.size_message.store(packed, Ordering::Release);
        }
    }
}

// SAFETY: `frame` returns the frame `HostCell::frame` composed, whose layer and mesh pointers the host keeps valid
// until its next frame, which only the next `frame` call asks for
#[allow(unsafe_code)]
unsafe impl OverlayHost for ManagerOverlayHost {
    fn game_ready(&self) -> bool {
        self.ctx.game_alive()
    }

    fn frame(&self, input: &AmmHostFrameInput) -> Option<&AmmHostFrame> {
        self.frames.fetch_add(1, Ordering::Relaxed);
        let input = AmmHostFrameInput { menu_screen: self.menu.load(Ordering::Relaxed), ..*input };
        let composed = match self.cell.frame(&input) {
            Ok(composed) => composed,
            Err(status) => {
                if !self.failure_logged.swap(true, Ordering::Relaxed) {
                    let status = format!("{status:?}");
                    self.ctx
                        .log()
                        .warn("host frame failed, no layers drawn until it works again", &[("status", &status)]);
                }
                return None;
            }
        };
        self.failure_logged.store(false, Ordering::Relaxed);
        self.publish(&composed);
        let cell = self.frame.get();
        // SAFETY: only this call writes the cell, and the renderer that called it is done with the frame the cell held
        // (it asks for a new frame only after it drew the last one); the reference lives until the next `frame` call
        unsafe {
            cell.write(composed);
            Some(&*cell)
        }
    }

    fn textures_lost(&self) {
        let _ = self.cell.with(|runtime| {
            runtime.textures_lost();
            Ok(())
        });
    }

    fn frame_skipped(&self) {
        self.skipped.fetch_add(1, Ordering::Relaxed);
    }
}
