/// Descriptor texts for tests.
pub struct Descriptors;

impl Descriptors {
    pub const FREECAM: &'static str = include_str!("../../../amm-core/fixtures/four_mods/freecam.hcl");
    pub const DEJAVU: &'static str = include_str!("../../../amm-core/fixtures/four_mods/dejavu.hcl");

    /// A descriptor with the given claims (capability, mode) and extra HCL after them.
    pub fn with_claims(id: &str, claims: &[(&str, &str)], extra: &str) -> String {
        let mut text = format!(
            "namespace = \"test\"\nname = \"{id}\"\ndisplay_name = \"{id}\"\nversion = \"0.1.0\"\nauthor = \"A\"\nconfig_path = \"config.hcl\"\n"
        );
        for (capability, mode) in claims {
            text.push_str(&format!("claim \"{capability}\" {{\n  mode = \"{mode}\"\n  reason = \"r\"\n}}\n"));
        }
        text.push_str(extra);
        text
    }

    /// A descriptor with a shared Overlay claim.
    pub fn overlay(id: &str) -> String {
        Self::with_claims(id, &[("Overlay", "Shared")], "")
    }
}
