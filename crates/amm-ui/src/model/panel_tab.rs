/// The tabs of a mod's page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PanelTab {
    #[default]
    General,
    Keybinds,
    Capabilities,
}
