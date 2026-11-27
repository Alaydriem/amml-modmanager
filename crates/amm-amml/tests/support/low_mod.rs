use amml_api::{AmmlMod, ApiError, Ctx};

/// A third mod of the fake loader, `@test/low`, for its own input leases.
pub struct LowMod;

impl AmmlMod for LowMod {
    const NAMESPACE: &'static str = "test";
    const NAME: &'static str = "low";

    fn enable(_ctx: &Ctx) -> Result<(), ApiError> {
        Ok(())
    }
}
