use amml_i18n::service::PackSource;

/// The compiled language packs of `AmmMessage`, built into the manager DLL. Each file in `i18n/packs/` is listed
/// here; a test fails when one is missing.
pub struct AmmPacks;

impl AmmPacks {
    pub const SOURCE: PackSource = PackSource::Embedded(&[("en_XA", include_bytes!("../../i18n/packs/en_XA.json"))]);
}
