use std::sync::{Mutex, MutexGuard, PoisonError};

use amm_client::link::ManagerLink;
use amm_client::test_support::FakeTable;
use amml_api::{AmmlMod, ApiError, Ctx, DisableCtx};

/// A consumer mod for whole-mod tests: `enable` starts a link with `FakeTable::DESCRIPTOR`, `disable` drops it.
pub struct ConsumerMod;

static LINK: Mutex<Option<ManagerLink>> = Mutex::new(None);

impl ConsumerMod {
    /// The link the last `enable` started, `None` after `disable`.
    pub fn link() -> MutexGuard<'static, Option<ManagerLink>> {
        LINK.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl AmmlMod for ConsumerMod {
    const NAMESPACE: &'static str = "test";
    const NAME: &'static str = "alaydriem-test";

    fn enable(ctx: &Ctx) -> Result<(), ApiError> {
        let link = ManagerLink::start_without_frame(ctx, FakeTable::DESCRIPTOR).map_err(|_| ApiError::Refused)?;
        *Self::link() = Some(link);
        Ok(())
    }

    fn disable(_ctx: &Ctx, _how: &DisableCtx) {
        let link = Self::link().take();
        drop(link);
    }
}
