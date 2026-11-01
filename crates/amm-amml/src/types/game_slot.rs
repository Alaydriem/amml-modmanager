/// A vtable slot of a game object the manager reads at enable and calls through later. The first five are required; each of the others turns off only its feature when it is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameSlot {
    ReleaseMouse,
    GrabMouse,
    LocalPlayer,
    WorldActive,
    ShowingMenu,
    TopScreenName,
    Options,
    SetHideGui,
    GetHideHud,
}

impl GameSlot {
    pub const COUNT: usize = 9;

    pub const ALL: [GameSlot; Self::COUNT] = [
        Self::ReleaseMouse,
        Self::GrabMouse,
        Self::LocalPlayer,
        Self::WorldActive,
        Self::ShowingMenu,
        Self::TopScreenName,
        Self::Options,
        Self::SetHideGui,
        Self::GetHideHud,
    ];

    pub fn entry(self) -> &'static str {
        match self {
            Self::ReleaseMouse => "IMinecraftGame.releaseMouse",
            Self::GrabMouse => "IMinecraftGame.grabMouse",
            Self::LocalPlayer => "ClientInstance.getLocalPlayer",
            Self::WorldActive => "ClientInstance.isWorldActive",
            Self::ShowingMenu => "ClientInstance.isShowingMenu",
            Self::TopScreenName => "ClientInstance.getTopScreenName",
            Self::Options => "ClientInstance.getOptions",
            Self::SetHideGui => "IOptionRegistry.setHideGUI",
            Self::GetHideHud => "IOptionRegistry.getHideHud",
        }
    }

    /// The feature a missing entry turns off; `None` for a required entry.
    pub fn feature(self) -> Option<&'static str> {
        match self {
            Self::ReleaseMouse | Self::GrabMouse | Self::LocalPlayer | Self::WorldActive | Self::ShowingMenu => None,
            Self::TopScreenName => Some("the menu screen"),
            Self::Options | Self::SetHideGui | Self::GetHideHud => Some("hide HUD"),
        }
    }
}
