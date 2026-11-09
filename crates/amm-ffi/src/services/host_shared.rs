use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use amm_abi::types::AmmHandle;
use amm_core::structs::registry::ModMask;
use amm_core::structs::render::RenderSize;
use amm_core::structs::snapshot::RenderSnapshot;
use arc_swap::ArcSwap;

use super::HolderBook;

/// The host state other threads read without the host lock: the render snapshot, the live generation of each
/// slot (for `log`), the holder book (for `holder`), the end of the last visible toast, whether the menu button
/// is placed, whether the player's turn is blocked, whether the gamepad is blocked, whether a screenshot capture is in flight, the owners
/// whose textures must be dropped, the forced render size (for `ResizeBuffers` and the window procedure), whether
/// the render size needs every frame, whether the game window changed since the last frame, and the back buffer of
/// the last Present. Every field is fixed in size.
pub struct HostShared {
    snapshot: Arc<ArcSwap<RenderSnapshot>>,
    generations: [AtomicU32; ModMask::CAPACITY],
    holders: ArcSwap<HolderBook>,
    toast_until_ms: AtomicU64,
    menu_button_placed: AtomicBool,
    blocks_player_turn: AtomicBool,
    blocks_gamepad: AtomicBool,
    capturing: AtomicBool,
    freed_owners: AtomicU32,
    // the forced render size packed by `pack`, 0 for none
    forced_size: AtomicU64,
    render_size_busy: AtomicBool,
    // the window procedure saw a DPI, display or position change of the game window while a size was forced
    window_changed: AtomicBool,
    // the back buffer of the last Present packed by `pack`, and its DXGI format; the two are written one after the
    // other, so a reader may see the size of one Present with the format of the one before
    back_buffer: AtomicU64,
    back_buffer_format: AtomicU32,
}

impl HostShared {
    const LIVE: u32 = 1 << 16;
    const WIDTH_SHIFT: u32 = 32;

    pub fn new(snapshot: Arc<ArcSwap<RenderSnapshot>>, holders: Arc<HolderBook>) -> Self {
        Self {
            snapshot,
            generations: std::array::from_fn(|_| AtomicU32::new(0)),
            holders: ArcSwap::new(holders),
            toast_until_ms: AtomicU64::new(0),
            menu_button_placed: AtomicBool::new(false),
            blocks_player_turn: AtomicBool::new(false),
            blocks_gamepad: AtomicBool::new(false),
            capturing: AtomicBool::new(false),
            freed_owners: AtomicU32::new(0),
            forced_size: AtomicU64::new(0),
            render_size_busy: AtomicBool::new(false),
            window_changed: AtomicBool::new(false),
            back_buffer: AtomicU64::new(0),
            back_buffer_format: AtomicU32::new(0),
        }
    }

    pub fn snapshot(&self) -> &ArcSwap<RenderSnapshot> {
        &self.snapshot
    }

    /// Marks a slot live with its generation, or free.
    pub fn set_generation(&self, slot: u16, generation: Option<u16>) {
        if let Some(cell) = self.generations.get(usize::from(slot)) {
            cell.store(generation.map_or(0, |generation| Self::LIVE | u32::from(generation)), Ordering::SeqCst);
        }
    }

    /// True when the handle names a live registration.
    pub fn is_live(&self, handle: AmmHandle) -> bool {
        self.generations
            .get(usize::from(handle.slot))
            .is_some_and(|cell| cell.load(Ordering::SeqCst) == Self::LIVE | u32::from(handle.generation))
    }

    pub fn holders(&self) -> &ArcSwap<HolderBook> {
        &self.holders
    }

    pub fn set_toast_until(&self, until_ms: u64) {
        self.toast_until_ms.store(until_ms, Ordering::Relaxed);
    }

    /// True while a toast may still be on screen at `now_ms`.
    pub fn toast_visible(&self, now_ms: u64) -> bool {
        now_ms < self.toast_until_ms.load(Ordering::Relaxed)
    }

    pub fn set_menu_button_placed(&self, placed: bool) {
        self.menu_button_placed.store(placed, Ordering::Relaxed);
    }

    /// True while the manager routes the pointer to a menu button it drew, so the frame that hides the button
    /// must reach the host to take it away.
    pub fn menu_button_placed(&self) -> bool {
        self.menu_button_placed.load(Ordering::Relaxed)
    }

    pub fn set_blocks_player_turn(&self, blocks: bool) {
        self.blocks_player_turn.store(blocks, Ordering::Relaxed);
    }

    /// True while an active Input holder asks for the player's own turn to be blocked.
    pub fn blocks_player_turn(&self) -> bool {
        self.blocks_player_turn.load(Ordering::Relaxed)
    }

    pub fn set_blocks_gamepad(&self, blocks: bool) {
        self.blocks_gamepad.store(blocks, Ordering::Relaxed);
    }

    /// True while the active exclusive Input holder has a `full_block` claim.
    pub fn blocks_gamepad(&self) -> bool {
        self.blocks_gamepad.load(Ordering::Relaxed)
    }

    pub fn set_capturing(&self, capturing: bool) {
        self.capturing.store(capturing, Ordering::Relaxed);
    }

    /// True while a capture waits to be offered to the overlay or for its report, so every Present must reach the
    /// host.
    pub fn capturing(&self) -> bool {
        self.capturing.load(Ordering::Relaxed)
    }

    pub fn mark_freed(&self, slot: u16) {
        if let Some(bit) = 1u32.checked_shl(u32::from(slot)) {
            self.freed_owners.fetch_or(bit, Ordering::SeqCst);
        }
    }

    /// The owners freed since the last call.
    pub fn take_freed(&self) -> u32 {
        self.freed_owners.swap(0, Ordering::SeqCst)
    }

    pub fn set_forced_size(&self, size: Option<RenderSize>) {
        self.forced_size.store(size.map_or(0, Self::pack), Ordering::SeqCst);
    }

    /// The render size a `RenderSize` holder forces now.
    pub fn forced_size(&self) -> Option<RenderSize> {
        let size = Self::unpack(self.forced_size.load(Ordering::SeqCst));
        (!size.is_empty()).then_some(size)
    }

    pub fn mark_window_changed(&self) {
        self.window_changed.store(true, Ordering::Relaxed);
    }

    /// True once after `mark_window_changed`.
    pub fn take_window_changed(&self) -> bool {
        self.window_changed.swap(false, Ordering::Relaxed)
    }

    pub fn set_render_size_busy(&self, busy: bool) {
        self.render_size_busy.store(busy, Ordering::Relaxed);
    }

    /// True while a render size is forced or a size message waits, so every Present must reach the host.
    pub fn render_size_busy(&self) -> bool {
        self.render_size_busy.load(Ordering::Relaxed)
    }

    pub fn set_back_buffer(&self, size: RenderSize, format: u32) {
        self.back_buffer.store(Self::pack(size), Ordering::Relaxed);
        self.back_buffer_format.store(format, Ordering::Relaxed);
    }

    /// The back buffer size and DXGI format of the last Present, 0 x 0 and 0 before the first.
    pub fn back_buffer(&self) -> (RenderSize, u32) {
        (Self::unpack(self.back_buffer.load(Ordering::Relaxed)), self.back_buffer_format.load(Ordering::Relaxed))
    }

    fn pack(size: RenderSize) -> u64 {
        (u64::from(size.width) << Self::WIDTH_SHIFT) | u64::from(size.height)
    }

    fn unpack(packed: u64) -> RenderSize {
        RenderSize::new((packed >> Self::WIDTH_SHIFT) as u32, packed as u32)
    }
}
