use amm_ffi::api::ManagerTables;
use amml_api::{AmmlMod, ApiError, Ctx};

/// A mod that provides the manager tables in `enable`, as the manager mod does.
pub struct ManagerProvider;

impl AmmlMod for ManagerProvider {
    const NAMESPACE: &'static str = "amml";
    const NAME: &'static str = "modmanager";

    fn enable(ctx: &Ctx) -> Result<(), ApiError> {
        ManagerTables::provide_all(&ctx.registry())
    }
}
