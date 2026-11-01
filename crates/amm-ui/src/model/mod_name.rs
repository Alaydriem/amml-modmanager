use amm_core::structs::registry::ModIndex;
use amml_i18n::text::{Args, Text, UserText};

use crate::AmmMessage;
use crate::model::PanelModel;

/// A mod's name as the panel shows it: the display name it registered with, or "Unknown mod" for a
/// slot that is free now.
pub struct ModName;

impl ModName {
    pub fn text(model: &PanelModel, index: ModIndex) -> Text {
        match model.registered(index) {
            Some(registered) => Text::user(UserText::from_model(registered.descriptor.display_name.clone())),
            None => Text::message(AmmMessage::UnknownMod),
        }
    }

    /// `args` with the name in the placeholder `name`.
    pub fn arg(model: &PanelModel, args: Args, name: &'static str, index: ModIndex) -> Args {
        match model.registered(index) {
            Some(registered) => args.user(name, UserText::from_model(registered.descriptor.display_name.clone())),
            None => args.message(name, AmmMessage::UnknownMod),
        }
    }
}
