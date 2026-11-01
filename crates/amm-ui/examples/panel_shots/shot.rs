use amm_ui::model::{PanelAction, PanelController, PanelPage, PanelTab, RebindTarget};

use crate::scene::Scene;

// One screenshot: its file name, the panel state it shows and the time its toasts are drawn at
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shot {
    ModList,
    General,
    Keybinds,
    Capabilities,
    Conflicts,
    Toast,
    AllKeybinds,
    ManagerSettings,
    MenuButton,
    MenuToast,
}

impl Shot {
    pub const ALL: [Shot; 10] = [
        Shot::ModList,
        Shot::General,
        Shot::Keybinds,
        Shot::Capabilities,
        Shot::Conflicts,
        Shot::Toast,
        Shot::AllKeybinds,
        Shot::ManagerSettings,
        Shot::MenuButton,
        Shot::MenuToast,
    ];

    pub fn file_name(self) -> &'static str {
        match self {
            Shot::ModList => "01-mod-list.png",
            Shot::General => "02-general.png",
            Shot::Keybinds => "03-keybinds.png",
            Shot::Capabilities => "04-capabilities.png",
            Shot::Conflicts => "05-conflicts.png",
            Shot::Toast => "06-toast.png",
            Shot::AllKeybinds => "07-all-keybinds.png",
            Shot::ManagerSettings => "08-manager-settings.png",
            Shot::MenuButton => "09-menu-button.png",
            Shot::MenuToast => "10-menu-button-toast.png",
        }
    }

    // only the toast shots are drawn while the scene's toasts are still on screen
    pub fn now_ms(self) -> u64 {
        if matches!(self, Shot::Toast | Shot::MenuToast) { Scene::TOAST_VIEW_MS } else { Scene::QUIET_MS }
    }

    // the menu button shots show a game menu with the panel closed
    pub fn is_menu(self) -> bool {
        matches!(self, Shot::MenuButton | Shot::MenuToast)
    }

    pub fn controller(self, scene: &Scene) -> PanelController {
        let mut controller = PanelController::new();
        let (page, tab) = match self {
            Shot::ModList => (Some(scene.freecam), PanelTab::General),
            Shot::General | Shot::Toast => (Some(scene.minimap), PanelTab::General),
            Shot::Keybinds => (Some(scene.freecam), PanelTab::Keybinds),
            Shot::Capabilities => (Some(scene.freecam), PanelTab::Capabilities),
            Shot::Conflicts | Shot::AllKeybinds | Shot::ManagerSettings | Shot::MenuButton | Shot::MenuToast => {
                (None, PanelTab::General)
            }
        };
        controller.on_action(&PanelAction::Select(page));
        controller.on_action(&PanelAction::SelectTab(tab));
        if self == Shot::Keybinds {
            controller.begin_rebind(RebindTarget::Mod { mod_index: scene.freecam, action: scene.freecam_cycle_mode });
        }
        if self == Shot::AllKeybinds {
            controller.show_page(PanelPage::Keybinds);
        }
        if self == Shot::ManagerSettings {
            controller.show_page(PanelPage::ManagerSettings);
        }
        controller
    }
}
