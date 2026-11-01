use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::registry::ModIndex;

use crate::types::{AmmAcquireOutcome, AmmAcquireResult};

impl From<AcquireOutcome> for AmmAcquireResult {
    fn from(value: AcquireOutcome) -> Self {
        match value {
            AcquireOutcome::Granted => Self { outcome: AmmAcquireOutcome::Granted, holder: Self::NO_HOLDER },
            AcquireOutcome::Denied { holder } => Self { outcome: AmmAcquireOutcome::Denied, holder: holder.0 },
            AcquireOutcome::Preempted => Self { outcome: AmmAcquireOutcome::Preempted, holder: Self::NO_HOLDER },
        }
    }
}

impl From<AmmAcquireResult> for AcquireOutcome {
    fn from(value: AmmAcquireResult) -> Self {
        match value.outcome {
            AmmAcquireOutcome::Granted => AcquireOutcome::Granted,
            AmmAcquireOutcome::Denied => AcquireOutcome::Denied { holder: ModIndex(value.holder) },
            AmmAcquireOutcome::Preempted => AcquireOutcome::Preempted,
        }
    }
}
