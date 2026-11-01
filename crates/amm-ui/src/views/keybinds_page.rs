use amml_i18n::text::Text;
use amml_ui::icon::Icon;
use amml_ui::tokens::{SpacingToken, spacing};
use amml_ui::widget::{ChipTone, PageHeader, PageHeaderLead, ScrollBar, SectionHeader};
use egui::{ScrollArea, Ui};

use crate::model::{PanelController, PanelIntent, PanelModel, RebindTarget};
use crate::views::{KeybindRowView, PanelHint};
use crate::{AmmId, AmmMessage};

/// The Keybinds page of the Manager group: every keybind of every mod in one table, grouped by mod,
/// with the panel toggle at the top. A click on a row waits for the next key press as its new chord.
pub struct KeybindsPage;

impl KeybindsPage {
    pub fn show(ui: &mut Ui, model: &PanelModel, controller: &PanelController, out: &mut Vec<PanelIntent>) {
        ui.spacing_mut().item_spacing.y = 0.0;
        PageHeader::new(PageHeaderLead::Tile(Icon::Keyboard, ChipTone::Accent), Text::message(AmmMessage::Keybinds))
            .subtitle(Text::message(AmmMessage::KeybindsSubtitle))
            .show(ui);
        ui.add_space(<spacing::L as SpacingToken>::VALUE);
        let rows = model.keybind_rows();
        let area = ScrollArea::vertical().id_salt(AmmId::Keybinds).auto_shrink([false, false]);
        ScrollBar::show(ui, area, false, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            KeybindRowView::header().show(ui);
            // the mod of the group drawn last; the panel toggle's group has none
            let mut group = None;
            for row in &rows {
                let owner = match row.target {
                    RebindTarget::Mod { mod_index, .. } => Some(mod_index),
                    RebindTarget::ManagerToggle => None,
                };
                if group != Some(owner) {
                    group = Some(owner);
                    SectionHeader::new(KeybindRowView::group(row)).show(ui);
                }
                let capturing = controller.rebinding() == Some(row.target);
                if KeybindRowView::show(ui, model, row, capturing).clicked() && !capturing {
                    out.push(PanelIntent::BeginRebind(row.target));
                }
            }
            ui.add_space(<spacing::M as SpacingToken>::VALUE);
            PanelHint::show(ui, &Text::message(AmmMessage::KeybindsPageHint));
        });
    }
}
