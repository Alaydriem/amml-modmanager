use amm_core::structs::descriptor::Capability;
use amm_core::structs::registry::ModIndex;
use amm_core::structs::toast::ToastKind;
use amml_i18n::text::{Args, Text, UserText};

use crate::AmmMessage;
use crate::model::{CapabilityText, ModName, PanelModel, ToastWords};

/// The words of a toast, built only while the toast is drawn.
pub struct ToastText;

impl ToastText {
    pub fn render(kind: &ToastKind, names: &PanelModel) -> ToastWords {
        match *kind {
            ToastKind::Denied { capability, holder, requester } => {
                let message =
                    Text::format(AmmMessage::RequestDenied, ModName::arg(names, Args::new(), "mod", requester));
                ToastWords {
                    title: Self::held_by(capability, holder, names),
                    message: Some(message),
                    hint: Some(Text::message(AmmMessage::HintConflicts)),
                }
            }
            ToastKind::Blocked { capability, holder, mod_index, action } => {
                let who = ModName::arg(names, Args::new(), "mod", mod_index);
                let message = match names.keybind(mod_index, action) {
                    Some(bind) => Text::format(
                        AmmMessage::KeyBlocked,
                        who.user("action", UserText::from_model(bind.label.clone())),
                    ),
                    None => Text::format(AmmMessage::AKeyBlocked, who),
                };
                ToastWords {
                    title: Self::held_by(capability, holder, names),
                    message: Some(message),
                    hint: Some(Text::message(AmmMessage::HintConflicts)),
                }
            }
            ToastKind::FrameDisabled(index) => ToastWords {
                title: Text::format(AmmMessage::StoppedDrawing, ModName::arg(names, Args::new(), "mod", index)),
                message: Some(Text::message(AmmMessage::FailedTooManyFrames)),
                hint: Some(Text::message(AmmMessage::HintRetry)),
            },
            ToastKind::SaveFailed => {
                ToastWords { title: Text::message(AmmMessage::SaveFailedTryAgain), message: None, hint: None }
            }
        }
    }

    fn held_by(capability: Capability, holder: ModIndex, names: &PanelModel) -> Text {
        let args = Args::new().message("capability", CapabilityText::name(capability));
        Text::format(AmmMessage::HeldBy, ModName::arg(names, args, "holder", holder))
    }
}
