use amml_api::{AmmlMod, ApiError, Ctx};

/// A second mod of the fake loader, `@test/high`, for its own input leases.
pub struct HighMod;

impl AmmlMod for HighMod {
    const NAMESPACE: &'static str = "test";
    const NAME: &'static str = "high";

    fn enable(_ctx: &Ctx) -> Result<(), ApiError> {
        Ok(())
    }
}
