use amm_core::services::descriptor::DescriptorChecker;
use amm_core::structs::descriptor::{ClaimMode, InputClaim};

const HEADLESS: &str = include_str!("../../../fixtures/four_mods/headless.hcl");
const LEVELS: &str = include_str!("../../../fixtures/four_mods/levels.hcl");

fn checker() -> DescriptorChecker {
    DescriptorChecker::new(128, 256)
}

#[test]
fn the_input_claim_carries_its_flags_and_the_passthrough_keys() {
    let descriptor = checker().parse(HEADLESS).expect("parses");

    assert_eq!(
        descriptor.input_claim(),
        Some(InputClaim {
            mode: ClaimMode::Exclusive,
            block_player_turn: false,
            pass_pointer_motion: true,
            full_block: true,
            passthrough: vec![87, 114],
        })
    );
}

#[test]
fn a_mod_without_an_input_claim_has_none() {
    let descriptor = checker().parse(LEVELS).expect("parses");

    assert_eq!(descriptor.input_claim(), None);
}
