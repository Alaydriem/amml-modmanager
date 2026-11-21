/// Descriptor texts of the mods a test registers with the manager.
pub struct Descriptors;

impl Descriptors {
    /// A mod with one shared `Overlay` claim and one global key bind on `chord` (index 0), saved at `keys.act`.
    pub fn with_bind(name: &str, chord: &str) -> String {
        let claim = "claim \"Overlay\" {\n  mode = \"Shared\"\n  reason = \"r\"\n}\n";
        let keybind = format!(
            "keybind \"act\" {{\n  label = \"Act\"\n  default = \"{chord}\"\n  capability = \"Overlay\"\n  scope = \"Global\"\n  config_path = \"keys.act\"\n}}\n"
        );
        let section = format!(
            "section \"keys\" {{\n  title = \"Keys\"\n  field \"keys.act\" {{\n    label = \"Act\"\n    kind = \"chord\"\n    default = \"{chord}\"\n    apply = \"Live\"\n  }}\n}}\n"
        );
        format!("{}{claim}{keybind}{section}", Self::head(name))
    }

    /// A mod with one shared `Hud` claim that hides the vanilla HUD.
    pub fn hud(name: &str) -> String {
        format!(
            "{}claim \"Hud\" {{\n  mode = \"Shared\"\n  reason = \"r\"\n  hide_vanilla_hud = true\n}}\n",
            Self::head(name)
        )
    }

    /// A mod with one shared `Overlay` claim and one exclusive `RenderSize` claim.
    pub fn overlay_and_size(name: &str) -> String {
        format!(
            "{}claim \"Overlay\" {{\n  mode = \"Shared\"\n  reason = \"r\"\n}}\nclaim \"RenderSize\" {{\n  mode = \"Exclusive\"\n  reason = \"r\"\n}}\n",
            Self::head(name)
        )
    }

    fn head(name: &str) -> String {
        format!(
            "namespace = \"test\"\nname = \"{name}\"\ndisplay_name = \"{name}\"\nversion = \"0.1.0\"\nauthor = \"A\"\nconfig_path = \"config.hcl\"\n"
        )
    }
}
