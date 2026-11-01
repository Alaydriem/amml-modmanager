use crate::structs::descriptor::{Capability, ClaimMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub capability: Capability,
    pub mode: ClaimMode,
    pub reason: String,
    // only meaningful on a Hud claim
    pub hide_vanilla_hud: bool,
    // only meaningful on an Input claim: the player's own turn is blocked while the claim is active
    pub block_player_turn: bool,
    // only meaningful on an Input claim: the game keeps the pointer motion while the claim is active
    pub pass_pointer_motion: bool,
}
