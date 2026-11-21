use amml_abi::AmmlReleaseReason;

/// Why the loader let go of the input a mod injected through its input lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseReason {
    FocusLost,
    WorldLeft,
    /// Another lease preempts or outranks the mod's lease, for example the open manager panel.
    Suspended,
    LeaseEnded,
    /// Another lease set the movement.
    Replaced,
    Disabled,
    /// A real key or button up, or a key reset of the game, ended one held key or button in the game.
    Reset,
    /// A reason this build does not know, with its loader value.
    Other(u8),
}

impl From<AmmlReleaseReason> for ReleaseReason {
    fn from(reason: AmmlReleaseReason) -> Self {
        match reason {
            AmmlReleaseReason::FOCUS_LOST => Self::FocusLost,
            AmmlReleaseReason::WORLD_LEFT => Self::WorldLeft,
            AmmlReleaseReason::SUSPENDED => Self::Suspended,
            AmmlReleaseReason::LEASE_ENDED => Self::LeaseEnded,
            AmmlReleaseReason::REPLACED => Self::Replaced,
            AmmlReleaseReason::DISABLED => Self::Disabled,
            AmmlReleaseReason::RESET => Self::Reset,
            AmmlReleaseReason(other) => Self::Other(other),
        }
    }
}
