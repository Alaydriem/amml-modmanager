use amm_core::services::input::CornerRow;
use amm_core::structs::menu::{CornerSlot, CornerTarget};
use amml_i18n::text::{Text, UserText};
use amml_ui::icon::Icon;
use amml_ui::widget::{CornerButton, CornerButtonLead};
use egui::{Rect, Ui, UiBuilder, pos2};

use crate::model::{PanelAction, PanelIntent, PanelModel};

/// The corner row in the bottom-right while the game shows its own cursor: the square Mods button, then each
/// mod's menu entry for this screen to its left. It sits above the game's version text, so it never covers it.
/// A click on Mods asks to open the panel; a click on an entry asks to tell its mod.
pub struct CornerRowView;

impl CornerRowView {
    /// Draws the row and returns the rect around it, in points.
    pub fn show(ui: &mut Ui, screen: Rect, model: &PanelModel, out: &mut Vec<PanelIntent>) -> Option<Rect> {
        let entries: Vec<(CornerTarget, f32)> = model
            .menu_entries()
            .map(|entry| {
                let target = CornerTarget::Entry { owner: entry.owner, id: entry.id };
                (target, Self::entry_button(&entry.mark, &entry.label).width(ui.ctx()))
            })
            .collect();
        let mut slots: Vec<CornerSlot> = Vec::with_capacity(entries.len() + 1);
        CornerRow::layout(screen.width(), screen.height(), &entries, &mut slots);
        for slot in &slots {
            let [min_x, min_y, max_x, max_y] = slot.rect;
            let rect = Rect::from_min_max(pos2(min_x, min_y), pos2(max_x, max_y));
            let button = match slot.target {
                CornerTarget::Mods => CornerButton::new(CornerButtonLead::Icon(Icon::Mods)),
                CornerTarget::Entry { owner, id } => {
                    let entry = model.menu_entries().find(|entry| entry.owner == owner && entry.id == id);
                    let (mark, label) = entry.map_or(("", ""), |entry| (entry.mark.as_str(), entry.label.as_str()));
                    Self::entry_button(mark, label)
                }
            };
            // a child ui, so the row never moves the cursor of the screen under it
            let clicked = button.show(&mut ui.new_child(UiBuilder::new().max_rect(rect))).clicked();
            if clicked {
                let action = match slot.target {
                    CornerTarget::Mods => PanelAction::Open,
                    CornerTarget::Entry { owner, id } => PanelAction::MenuEntry { owner, id },
                };
                out.push(PanelIntent::Action(action));
            }
        }
        CornerRow::bounds(&slots)
            .map(|[min_x, min_y, max_x, max_y]| Rect::from_min_max(pos2(min_x, min_y), pos2(max_x, max_y)))
    }

    // a mod's entry: its mark and its label, both as the mod gave them
    fn entry_button(mark: &str, label: &str) -> CornerButton {
        CornerButton::new(CornerButtonLead::Mark(UserText::from_model(mark.to_owned())))
            .label(Text::user(UserText::from_model(label.to_owned())))
    }
}
