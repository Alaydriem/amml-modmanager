/// Descriptor texts for the fixtures.
pub struct Descriptors;

impl Descriptors {
    pub const FREECAM: &'static str = include_str!("../../../amm-core/fixtures/four_mods/freecam.hcl");
    pub const DEJAVU: &'static str = include_str!("../../../amm-core/fixtures/four_mods/dejavu.hcl");
    pub const LEVELS: &'static str = include_str!("../../../amm-core/fixtures/four_mods/levels.hcl");
    pub const MINIMAP: &'static str = include_str!("../../../amm-core/fixtures/four_mods/minimap.hcl");

    const HEADER_TAIL: &'static str = "version = \"0.1.0\"\nauthor = \"Bench\"\nconfig_path = \"config.hcl\"\n";

    /// A descriptor with one claim per `(capability, mode)` pair and no keybinds.
    pub fn with_claims(id: &str, claims: &[(&str, &str)]) -> String {
        let mut text = format!("namespace = \"test\"\nname = \"{id}\"\ndisplay_name = \"{id}\"\n{}", Self::HEADER_TAIL);
        for (capability, mode) in claims {
            text.push_str(&format!("claim \"{capability}\" {{\n  mode = \"{mode}\"\n  reason = \"bench\"\n}}\n"));
        }
        text
    }

    /// A descriptor with shared Overlay and WorldDraw claims and one Global keybind per chord text, each on
    /// WorldDraw.
    pub fn with_binds(id: &str, chords: &[String]) -> String {
        let mut text = Self::with_claims(id, &[("Overlay", "Shared"), ("WorldDraw", "Shared")]);
        let mut fields = String::new();
        for (n, chord) in chords.iter().enumerate() {
            text.push_str(&format!(
                "keybind \"bind{n}\" {{\n  label = \"bind{n}\"\n  default = \"{chord}\"\n  capability = \"WorldDraw\"\n  \
                 scope = \"Global\"\n  config_path = \"keys.bind{n}\"\n}}\n"
            ));
            fields.push_str(&format!(
                "  field \"keys.bind{n}\" {{\n    label = \"bind{n}\"\n    kind = \"chord\"\n    default = \"{chord}\"\n    \
                 apply = \"Live\"\n  }}\n"
            ));
        }
        text.push_str(&format!("section \"keys\" {{\n  title = \"Keys\"\n{fields}}}\n"));
        text
    }
}
