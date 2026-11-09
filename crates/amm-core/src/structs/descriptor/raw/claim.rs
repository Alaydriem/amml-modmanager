use serde::Deserialize;

use crate::structs::descriptor::{Capability, Claim, ClaimMode};

#[derive(Debug, Deserialize)]
pub struct RawClaim {
    pub mode: ClaimMode,
    pub reason: String,
    #[serde(default)]
    pub hide_vanilla_hud: bool,
    #[serde(default)]
    pub block_player_turn: bool,
    #[serde(default)]
    pub pass_pointer_motion: bool,
    #[serde(default)]
    pub full_block: bool,
}

impl RawClaim {
    pub fn into_claim(self, capability: Capability) -> Claim {
        Claim {
            capability,
            mode: self.mode,
            reason: self.reason,
            hide_vanilla_hud: self.hide_vanilla_hud,
            block_player_turn: self.block_player_turn,
            pass_pointer_motion: self.pass_pointer_motion,
            full_block: self.full_block,
        }
    }
}
