/// Descriptor texts for the scenarios. Texts built at run time are leaked, because a link keeps its descriptor
/// for the life of the process.
pub struct Descriptors;

impl Descriptors {
    pub const FREECAM: &'static str = include_str!("../../../amm-core/fixtures/four_mods/freecam.hcl");
    pub const DEJAVU: &'static str = include_str!("../../../amm-core/fixtures/four_mods/dejavu.hcl");

    /// A descriptor with the given claims (capability, mode) followed by `extra` HCL.
    pub fn with_claims(id: &str, claims: &[(&str, &str)], extra: &str) -> &'static str {
        let mut text = format!(
            "namespace = \"test\"\nname = \"{id}\"\ndisplay_name = \"{id}\"\nversion = \"0.1.0\"\nauthor = \"A\"\nconfig_path = \"config.hcl\"\n"
        );
        for (capability, mode) in claims {
            text.push_str(&format!("claim \"{capability}\" {{\n  mode = \"{mode}\"\n  reason = \"r\"\n}}\n"));
        }
        text.push_str(extra);
        Box::leak(text.into_boxed_str())
    }

    /// A Global keybind on `capability` whose chord is a setting at `keys.<name>`.
    pub fn chord_bind(name: &str, capability: &str, chord: &str) -> String {
        format!(
            "keybind \"{name}\" {{\n  label = \"{name}\"\n  default = \"{chord}\"\n  capability = \"{capability}\"\n  \
             scope = \"Global\"\n  config_path = \"keys.{name}\"\n}}\n\
             section \"keys\" {{\n  title = \"Keys\"\n  field \"keys.{name}\" {{\n    label = \"{name}\"\n    \
             kind = \"chord\"\n    default = \"{chord}\"\n    apply = \"Live\"\n  }}\n}}\n"
        )
    }
}
