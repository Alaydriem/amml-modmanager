use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use amm_abi::convert::move_intent::MoveIntentCodec;
use amm_core::structs::input::{MoveIntent, MoveLayout};

use crate::traits::GameCalls;

/// Writes the held move intent into the game's `RawMoveInputComponent` after `extractRawHIDInput` filled it, so the
/// local player moves with no keyboard and the game builds its own movement packet from it.
///
/// The tick (`publish`) reads the intent from the host and keeps it in one atomic, packed; it keeps 0 while there is
/// nothing to write: no intent, no local player in an active world, or a screen open over play. The detour (`write`)
/// does one load of that atomic and returns at once on 0, so it costs one atomic load per call with no intent and
/// never allocates. Only the held bits the layout owns, the analog vector and the raw move are written; edge bits
/// stay as the game set them. A clear needs no zero write: the game fills the component again on its next call.
pub struct FakeMoveWriter {
    // the packed intent of the last tick; 0 while nothing is to be written. Relaxed: the value is self-contained
    intent: AtomicU64,
    // set at the first enable whose entries resolved; the game build, and so the layout, does not change in a process
    layout: OnceLock<MoveLayout>,
}

impl FakeMoveWriter {
    const ON: u64 = 1 << 63;
    const AXIS_BITS: u32 = 16;
    const FLAGS_SHIFT: u32 = 32;

    pub const fn new() -> Self {
        Self { intent: AtomicU64::new(0), layout: OnceLock::new() }
    }

    /// Keeps `layout` when it passes `MoveLayout::checked`. True when the writer has that layout, so the move
    /// command is available.
    pub fn set_layout(&self, layout: Option<MoveLayout>) -> bool {
        let Some(layout) = layout.and_then(MoveLayout::checked) else {
            return false;
        };
        let kept = self.layout.get_or_init(|| layout);
        *kept == layout
    }

    /// Once per tick, after the host tick: tells the host whether `client` has a local player in an active world,
    /// and keeps the intent it answers while that player is in play with no screen open. Does nothing without a
    /// layout.
    pub fn publish(&self, calls: &dyn GameCalls, client: usize) {
        if self.layout.get().is_none() {
            return;
        }
        let ready = client != 0 && calls.local_player(client) != 0 && calls.world_active(client);
        let packed = match calls.host_move(ready) {
            Some(intent) if ready && !calls.showing_menu(client) => Self::pack(intent),
            _ => 0,
        };
        self.intent.store(packed, Ordering::Relaxed);
    }

    /// The detour body: writes the kept intent into the component at `raw`, the `RawMoveInputComponent&` the game
    /// passed to `extractRawHIDInput`. Nothing is written with no intent, no layout or a `raw` of 0.
    #[allow(unsafe_code)]
    pub fn write(&self, raw: usize) {
        let packed = self.intent.load(Ordering::Relaxed);
        if packed == 0 || raw == 0 {
            return;
        }
        let Some(layout) = self.layout.get() else {
            return;
        };
        let write = layout.write_for(Self::unpack(packed));
        let state = raw + layout.input_state as usize;
        let flags = state as *mut u32;
        let analog = (state + layout.analog as usize) as *mut [f32; 2];
        let raw_move = (raw + layout.raw_move as usize) as *mut [f32; 2];
        // SAFETY: `raw` is the live RawMoveInputComponent the game passed for this call, and `MoveLayout::checked`
        // keeps the flag word and both vectors inside its `COMPONENT_SIZE` bytes, apart from each other; unaligned
        // accesses make no alignment assumption about the offsets
        unsafe {
            flags.write_unaligned((flags.read_unaligned() & !write.clear) | write.set);
            analog.write_unaligned(write.vector);
            raw_move.write_unaligned(write.vector);
        }
    }

    /// Nothing is written from now until the next `publish`.
    pub fn stop(&self) {
        self.intent.store(0, Ordering::Relaxed);
    }

    // the send_move values in one word: forward and left as 16-bit thousandths, the flags above them, and the ON
    // bit so an all-zero intent still differs from "nothing to write"
    fn pack(intent: MoveIntent) -> u64 {
        let (forward, left, flags) = MoveIntentCodec::encode(intent);
        let axis = |value: i32| u64::from(value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16 as u16);
        Self::ON | axis(forward) | (axis(left) << Self::AXIS_BITS) | (u64::from(flags) << Self::FLAGS_SHIFT)
    }

    fn unpack(packed: u64) -> MoveIntent {
        let forward = i32::from(packed as u16 as i16);
        let left = i32::from((packed >> Self::AXIS_BITS) as u16 as i16);
        let flags = ((packed & !Self::ON) >> Self::FLAGS_SHIFT) as u32;
        MoveIntentCodec::decode(forward, left, flags).unwrap_or(MoveIntent::ZERO)
    }
}

impl Default for FakeMoveWriter {
    fn default() -> Self {
        Self::new()
    }
}
