use amm_core::errors::LinkError;
use amml_api::amml_abi::{
    AmmlEvent, AmmlUseRow, InjectedInputReleased, InputLeaseLost, InputLeaseResumed, InputLeaseSuspended, ModsLoaded,
    ProviderGone, ProviderReady, ScreenshotTaken, ScreenshotWritten,
};
use amml_api::{Area, Uses};

/// The rows a mod's `USES` needs for its `ManagerLink`. A mod calls `check` in a test, so a missing row fails there
/// and not in the game.
pub struct LinkUses;

impl LinkUses {
    /// The events the link cannot work without: each must be a `required` row.
    pub const REQUIRED: [&'static str; 2] = [ProviderReady::ID, ProviderGone::ID];

    /// The events and areas the link uses when the loader serves them: each must be a row, `required` or `optional`.
    /// Without `input.lease`, `acquire(Capability::Input)` gives `Api(NotAvailable)`.
    pub const OPTIONAL: [&'static str; 8] = [
        Area::INPUT_LEASE.id,
        ModsLoaded::ID,
        InputLeaseSuspended::ID,
        InputLeaseResumed::ID,
        InputLeaseLost::ID,
        InjectedInputReleased::ID,
        ScreenshotTaken::ID,
        ScreenshotWritten::ID,
    ];

    /// `Err(MissingUse(id))` for the first id of `REQUIRED` that `uses` has no required row for, or the first id of
    /// `OPTIONAL` it has no row for.
    pub fn check(uses: &Uses) -> Result<(), LinkError> {
        let rows: Vec<(&str, bool)> = uses.rows().iter().map(|row| (Self::name(row), row.required != 0)).collect();
        let missing_required =
            Self::REQUIRED.iter().find(|id| !rows.iter().any(|(name, required)| name == *id && *required));
        let missing_optional = Self::OPTIONAL.iter().find(|id| !rows.iter().any(|(name, _)| name == *id));
        match missing_required.or(missing_optional) {
            Some(id) => Err(LinkError::MissingUse((*id).to_owned())),
            None => Ok(()),
        }
    }

    fn name(row: &AmmlUseRow) -> &'static str {
        // SAFETY: the rows of a `Uses` are built by `amml_uses!` from `&'static str` names
        let name: Result<&'static str, _> = unsafe { row.name.as_str() };
        name.unwrap_or("")
    }
}
