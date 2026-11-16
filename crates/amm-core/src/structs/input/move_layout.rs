use super::{MoveIntent, MoveWrite};

/// Where a move intent goes in the game's `RawMoveInputComponent`, from the `RawMoveInputComponent.*` and
/// `MoveInputState.*` entries of the manager's entry list. Offsets are in bytes: `input_state` and `raw_move` from
/// the start of the component, `analog` from the start of the `MoveInputState`. The flag word is the first `u32` of
/// the `MoveInputState`. `sneak_mask` is the game's sneak test, which also holds the Descend bit; only SneakDown
/// (bit 0, `SNEAK_DOWN`) is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveLayout {
    pub input_state: u32,
    pub raw_move: u32,
    pub analog: u32,
    pub sneak_mask: u32,
    pub jump_mask: u32,
    pub sprint_bit: u32,
    pub up_bit: u32,
    pub down_bit: u32,
    pub left_bit: u32,
    pub right_bit: u32,
}

impl MoveLayout {
    /// The size of `RawMoveInputComponent` on every build the entries derive on; no write leaves it.
    pub const COMPONENT_SIZE: u32 = 0x18;
    /// An axis past this, either way, also sets its direction bit; below it only the vector moves.
    pub const DIRECTION_DEAD_ZONE: f32 = 0.3;
    /// SneakDown, the one bit of the sneak test that sneak writes.
    pub const SNEAK_DOWN: u32 = 1;
    /// The jump mask the game uses on every build the entries derive on.
    pub const EXPECTED_JUMP_MASK: u32 = 0x80;
    /// The SprintDown, Up, Down, Left and Right bit numbers the game uses on every build the entries derive on.
    pub const EXPECTED_BITS: [u32; 5] = [8, 13, 14, 15, 16];
    const FLAGS_SIZE: u32 = 4;
    const VEC2_SIZE: u32 = 8;
    const OWNED_BITS: u32 = 7;

    /// The layout, when every write stays inside the component, the two vectors and the flag word do not overlap,
    /// every bit number is below 32, the jump mask is one bit, and the seven written bits are distinct.
    pub fn checked(self) -> Option<Self> {
        let flags = (self.input_state, self.input_state.checked_add(Self::FLAGS_SIZE)?);
        let analog_start = self.input_state.checked_add(self.analog)?;
        let analog = (analog_start, analog_start.checked_add(Self::VEC2_SIZE)?);
        let raw_move = (self.raw_move, self.raw_move.checked_add(Self::VEC2_SIZE)?);
        let ranges = [flags, analog, raw_move];
        if ranges.iter().any(|(_, end)| *end > Self::COMPONENT_SIZE) {
            return None;
        }
        let overlaps = |a: (u32, u32), b: (u32, u32)| a.0 < b.1 && b.0 < a.1;
        if overlaps(flags, analog) || overlaps(flags, raw_move) || overlaps(analog, raw_move) {
            return None;
        }
        let bits = [self.sprint_bit, self.up_bit, self.down_bit, self.left_bit, self.right_bit];
        if bits.iter().any(|bit| *bit >= u32::BITS) || self.sneak_mask == 0 || !self.jump_mask.is_power_of_two() {
            return None;
        }
        (self.owned().count_ones() == Self::OWNED_BITS).then_some(self)
    }

    /// The sneak test holds SneakDown, and the jump mask and the five flag bit numbers are the expected ones. The
    /// jump and bit entries have only one recipe each, so a different value is not trusted; a sneak test without
    /// SneakDown would make sneak write another bit. The flags are then not written, and only the vector moves the
    /// player.
    pub fn flags_as_expected(&self) -> bool {
        let bits = [self.sprint_bit, self.up_bit, self.down_bit, self.left_bit, self.right_bit];
        self.sneak_mask & Self::SNEAK_DOWN != 0
            && self.jump_mask == Self::EXPECTED_JUMP_MASK
            && bits == Self::EXPECTED_BITS
    }

    /// The write for `intent`: the held bits it owns are cleared, then set from the intent; the vector is
    /// `[left, forward]`, held to length 1. Edge bits ("was pressed") are never touched. When the flag numbers are
    /// not the expected ones, no flag is cleared or set. Call on a `checked` layout.
    pub fn write_for(&self, intent: MoveIntent) -> MoveWrite {
        let mut vector = [intent.left, intent.forward];
        let length = (vector[0] * vector[0] + vector[1] * vector[1]).sqrt();
        if length > 1.0 {
            vector = [vector[0] / length, vector[1] / length];
        }
        if !self.flags_as_expected() {
            return MoveWrite { clear: 0, set: 0, vector };
        }
        let zone = Self::DIRECTION_DEAD_ZONE;
        let set = Self::when(intent.sneak, Self::SNEAK_DOWN)
            | Self::when(intent.jump, self.jump_mask)
            | Self::when(intent.sprint, Self::bit(self.sprint_bit))
            | Self::when(intent.forward > zone, Self::bit(self.up_bit))
            | Self::when(intent.forward < -zone, Self::bit(self.down_bit))
            | Self::when(intent.left > zone, Self::bit(self.left_bit))
            | Self::when(intent.left < -zone, Self::bit(self.right_bit));
        MoveWrite { clear: self.owned(), set, vector }
    }

    fn owned(&self) -> u32 {
        Self::SNEAK_DOWN
            | self.jump_mask
            | Self::bit(self.sprint_bit)
            | Self::bit(self.up_bit)
            | Self::bit(self.down_bit)
            | Self::bit(self.left_bit)
            | Self::bit(self.right_bit)
    }

    fn bit(number: u32) -> u32 {
        1u32.checked_shl(number).unwrap_or(0)
    }

    fn when(on: bool, bits: u32) -> u32 {
        if on { bits } else { 0 }
    }
}
