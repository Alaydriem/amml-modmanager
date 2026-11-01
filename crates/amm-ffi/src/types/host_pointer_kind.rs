use amm_abi::types::AmmStatus;

/// What an `amm_host_pointer` call reports. `Move` uses `x` and `y`, `Button` also `button` and `down`, `Wheel`
/// uses `wheel` in lines, and `Delta` gives the raw mouse movement in `x` and `y`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmHostPointerKind {
    Move = 0,
    Button,
    Wheel,
    Delta,
}

impl TryFrom<u8> for AmmHostPointerKind {
    type Error = AmmStatus;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(AmmHostPointerKind::Move),
            1 => Ok(AmmHostPointerKind::Button),
            2 => Ok(AmmHostPointerKind::Wheel),
            3 => Ok(AmmHostPointerKind::Delta),
            _ => Err(AmmStatus::InvalidArgument),
        }
    }
}
