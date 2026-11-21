use amm_client::test_support::FakeTable;
use amml_api::{AmmlMod, ApiError, Ctx};

/// A manager mod for whole-mod tests: `enable` provides the five fake tables, as modmanager does.
pub struct ManagerMod;

impl AmmlMod for ManagerMod {
    const NAMESPACE: &'static str = "amml";
    const NAME: &'static str = "modmanager";

    fn enable(ctx: &Ctx) -> Result<(), ApiError> {
        FakeTable::provide(ctx)
    }
}
