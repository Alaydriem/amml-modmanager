use crate::structs::descriptor::ClaimMode;

/// A descriptor's `Input` claim, read by a client to make its one loader input lease: the claim mode, whether the
/// player's own turn is blocked, whether the game keeps the pointer motion, whether every input is kept from the game
/// (`full_block`, exclusive only), and the keys that still reach the game (`input_passthrough`, virtual-key codes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputClaim {
    pub mode: ClaimMode,
    pub block_player_turn: bool,
    pub pass_pointer_motion: bool,
    pub full_block: bool,
    pub passthrough: Vec<u32>,
}
