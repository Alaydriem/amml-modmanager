use amm_core::structs::write::SaveStatus;
use amml_i18n::text::{Args, Text, UserText};
use amml_ui::icon::Icon;
use amml_ui::widget::{Chip, ChipKind, ChipTone, IconButton, PageHeader, PageHeaderLead};
use egui::Ui;

use crate::AmmMessage;
use crate::model::{PanelAction, PanelIntent, PanelModel};

/// The panel's top bar, the panel form of the page header: the studio name over "Mod Manager", the
/// mod counts and the manager version, then the save status and close. Edits save on their own,
/// so the status says "Saving…" while a save waits, "Saved" just after it goes out and "Save failed"
/// when the writer refused it.
pub struct HeaderView;

impl HeaderView {
    /// The bar's height, which does not change with its text.
    pub fn height() -> f32 {
        Self::bar().height()
    }

    pub fn show(ui: &mut Ui, model: &PanelModel, out: &mut Vec<PanelIntent>) {
        let enabled = u64::try_from(model.enabled_count()).unwrap_or(u64::MAX);
        let installed = u64::try_from(model.mod_count()).unwrap_or(u64::MAX);
        let version = UserText::from_model(model.manager_version().to_owned());
        let meta = vec![
            Text::plural(AmmMessage::ModsInstalled, installed, Args::new()),
            Text::plural(AmmMessage::ModsEnabled, enabled, Args::new()),
            Text::format(AmmMessage::Version, Args::new().user("version", version)),
        ];
        let status = Self::status(model.save_status());
        Self::bar()
            .meta(meta)
            .trailing(|ui| {
                if IconButton::new(Icon::Close).show(ui).clicked() {
                    out.push(PanelIntent::Action(PanelAction::Close));
                }
                if let Some(chip) = status {
                    chip.show(ui);
                }
            })
            .show(ui);
    }

    fn bar<'a>() -> PageHeader<'a> {
        PageHeader::new(PageHeaderLead::Tile(Icon::Apps, ChipTone::Accent), Text::message(AmmMessage::ModManager))
            .eyebrow(Text::message(AmmMessage::Alaydriem))
    }

    fn status(status: SaveStatus) -> Option<Chip> {
        let chip = |tone, message| Chip::new(ChipKind::Display(tone), Text::message(message));
        match status {
            SaveStatus::Idle => None,
            SaveStatus::Saving => Some(chip(ChipTone::Outline, AmmMessage::Saving)),
            SaveStatus::Saved => Some(chip(ChipTone::Success, AmmMessage::Saved).icon(Icon::Check)),
            SaveStatus::Failed => Some(chip(ChipTone::Warning, AmmMessage::SaveFailed).icon(Icon::Warning)),
        }
    }
}
