use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use amm_client::link::ManagerLink;
use amml_api::{AmmlMod, ApiError, Ctx, DisableCtx};
use amml_managerprobe::{ProbeChecks, ProbeMod};

static CHECKS: Mutex<Option<(Arc<ProbeChecks>, Arc<AtomicU64>)>> = Mutex::new(None);

/// A mod whose `enable` starts a link with the probe's descriptor and builds `ProbeChecks` over it, with no loader
/// listeners, so a test drives the checks tick by tick.
pub struct ChecksMod;

impl ChecksMod {
    /// The checks the last `enable` built, with the frame counter of the layer they read.
    pub fn checks() -> MutexGuard<'static, Option<(Arc<ProbeChecks>, Arc<AtomicU64>)>> {
        CHECKS.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl AmmlMod for ChecksMod {
    const NAMESPACE: &'static str = "amml";
    const NAME: &'static str = "managerprobe";

    fn enable(ctx: &Ctx) -> Result<(), ApiError> {
        let link = ManagerLink::start_without_frame(ctx, ProbeMod::DESCRIPTOR).map_err(|_| ApiError::Refused)?;
        let frames = Arc::new(AtomicU64::new(0));
        *Self::checks() = Some((ProbeChecks::new_shared(*ctx, Arc::clone(&frames), link), frames));
        Ok(())
    }

    fn disable(_ctx: &Ctx, _how: &DisableCtx) {
        let checks = Self::checks().take();
        drop(checks);
    }
}
