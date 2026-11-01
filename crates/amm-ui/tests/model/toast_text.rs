use amm_core::structs::descriptor::Capability;
use amm_core::structs::toast::ToastKind;
use amm_ui::model::{PanelModel, ToastText, ToastWords};
use amml_i18n::localize::Localizer;
use amml_i18n::text::Text;

use crate::support::{FourMods, MINIMAP};

fn toggle_action(mods: &FourMods) -> u16 {
    let registered = mods.manager.registry().get(mods.freecam).unwrap_or_else(|| panic!("freecam is registered"));
    let position = registered.descriptor.keybinds.iter().position(|bind| bind.action == "toggle");
    position.unwrap_or_else(|| panic!("freecam has a toggle keybind")) as u16
}

// the three parts in English
fn english(words: &ToastWords) -> (String, Option<String>, Option<String>) {
    let localizer = Localizer::english();
    let resolve = |text: &Text| text.resolve(&localizer).into_owned();
    (resolve(&words.title), words.message.as_ref().map(resolve), words.hint.as_ref().map(resolve))
}

#[test]
fn blocked_toast_names_capability_holder_mod_and_action() {
    let mods = FourMods::new(MINIMAP);
    let kind = ToastKind::Blocked {
        capability: Capability::Camera,
        holder: mods.dejavu,
        mod_index: mods.freecam,
        action: toggle_action(&mods),
    };

    let words = ToastText::render(&kind, &PanelModel::new(&mods.manager, "1.4.1", 0));

    let (title, message, hint) = english(&words);
    assert_eq!(title, "Camera is held by DejaVu");
    assert_eq!(message.as_deref(), Some("Freecam: the Toggle freecam key was blocked."));
    assert_eq!(hint.as_deref(), Some("Change the rule in the panel under Conflicts"));
}

#[test]
fn denied_toast_names_capability_holder_and_requester() {
    let mods = FourMods::new(MINIMAP);
    let kind = ToastKind::Denied { capability: Capability::Camera, holder: mods.dejavu, requester: mods.freecam };

    let words = ToastText::render(&kind, &PanelModel::new(&mods.manager, "1.4.1", 0));

    let (title, message, _) = english(&words);
    assert_eq!(title, "Camera is held by DejaVu");
    assert_eq!(message.as_deref(), Some("Freecam: the request was denied."));
}

#[test]
fn save_failed_toast_is_a_title_alone() {
    let mods = FourMods::new(MINIMAP);

    let words = ToastText::render(&ToastKind::SaveFailed, &PanelModel::new(&mods.manager, "1.4.1", 0));

    assert_eq!(english(&words), ("Save failed, try again".to_owned(), None, None));
}
