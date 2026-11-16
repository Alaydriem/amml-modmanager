use amm_core::structs::input::MoveLayout;

use crate::traits::HookHost;

/// The game entries of the move command: the hooked `extractRawHIDInput` and the fields and flag numbers of the
/// `RawMoveInputComponent` it fills, read at enable. Every one is optional; one that is missing, or a set that does
/// not give a valid `MoveLayout`, turns off only the move command, with one log line. Flag numbers other than the
/// expected ones keep the command on with no flag writes, also with one log line.
pub struct MoveLayoutSlots;

impl MoveLayoutSlots {
    pub const HOOK: &'static str = "ClientInputUpdateSystem.extractRawHIDInput";
    /// The hook, then the fields in the order of `MoveLayout`.
    pub const ENTRIES: [&'static str; 11] = [
        Self::HOOK,
        "RawMoveInputComponent.mInputState",
        "RawMoveInputComponent.mRawMove",
        "MoveInputState.mAnalogMoveVector",
        "MoveInputState.maskSneakTest",
        "MoveInputState.maskJumpDown",
        "MoveInputState.bitSprintDown",
        "MoveInputState.bitUp",
        "MoveInputState.bitDown",
        "MoveInputState.bitLeft",
        "MoveInputState.bitRight",
    ];
    const FEATURE: &'static str = "the move command";

    /// The layout when every entry resolved and the values pass `MoveLayout::checked`; otherwise `None`, with one
    /// warning that names the first entry at fault.
    pub fn resolve<H: HookHost>(host: &H) -> Option<MoveLayout> {
        let mut values = [0u32; Self::ENTRIES.len()];
        for (value, entry) in values.iter_mut().zip(Self::ENTRIES) {
            let resolved = match host.resolve(entry) {
                Ok(resolved) => resolved,
                Err(error) => {
                    host.warn(&format!("{entry} did not resolve ({error}); {} is off", Self::FEATURE));
                    return None;
                }
            };
            if entry == Self::HOOK {
                continue;
            }
            let Ok(narrow) = u32::try_from(resolved) else {
                host.warn(&format!("{entry} is {resolved:#x}, past 32 bits; {} is off", Self::FEATURE));
                return None;
            };
            *value = narrow;
        }
        let [
            _,
            input_state,
            raw_move,
            analog,
            sneak_mask,
            jump_mask,
            sprint_bit,
            up_bit,
            down_bit,
            left_bit,
            right_bit,
        ] = values;
        let layout = MoveLayout {
            input_state,
            raw_move,
            analog,
            sneak_mask,
            jump_mask,
            sprint_bit,
            up_bit,
            down_bit,
            left_bit,
            right_bit,
        };
        let checked = layout.checked();
        match checked {
            None => host.warn(&format!("the move entries give no valid layout ({layout:?}); {} is off", Self::FEATURE)),
            Some(layout) if !layout.flags_as_expected() => host.warn(&format!(
                "the move flag entries are not the expected values ({layout:?}); {} writes no flags, only the move vector",
                Self::FEATURE
            )),
            Some(_) => {}
        }
        checked
    }
}
