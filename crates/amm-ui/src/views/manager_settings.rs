use amml_i18n::text::{Text, UserText};
use amml_ui::icon::Icon;
use amml_ui::tokens::{SpacingToken, spacing};
use amml_ui::widget::{
    ChipTone, Dropdown, DropdownOption, PageHeader, PageHeaderLead, ScrollBar, SectionCard, SettingControl, SettingRow,
};
use egui::{Id, ScrollArea, Ui};

use crate::model::{PanelAction, PanelIntent, PanelModel};
use crate::runtime::PanelLocale;
use crate::views::KeybindRowView;
use crate::{AmmId, AmmMessage};

/// The Manager settings page: the panel chord and the language every mod shows its text in.
pub struct ManagerSettingsView;

impl ManagerSettingsView {
    /// Paper: Mod manager screen 08, the language dropdown's width.
    const LANGUAGE_WIDTH: f32 = 250.0;

    pub fn show(ui: &mut Ui, model: &PanelModel, out: &mut Vec<PanelIntent>) {
        ui.spacing_mut().item_spacing.y = 0.0;
        let lead = PageHeaderLead::Tile(Icon::Settings, ChipTone::Accent);
        PageHeader::new(lead, Text::message(AmmMessage::ManagerSettings))
            .subtitle(Text::message(AmmMessage::ManagerSettingsSubtitle))
            .show(ui);
        ui.add_space(<spacing::L as SpacingToken>::VALUE);
        let area = ScrollArea::vertical().id_salt(AmmId::ManagerSettings).auto_shrink([false, false]);
        ScrollBar::show(ui, area, false, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let gap = <spacing::L as SpacingToken>::VALUE;
            Self::panel(ui, model);
            ui.add_space(gap);
            Self::language(ui, model, out);
        });
    }

    fn panel(ui: &mut Ui, model: &PanelModel) {
        let chord = KeybindRowView::chord(model.config().toggle_chord);
        SectionCard::new(Text::message(AmmMessage::PanelTitle)).show(ui, |rows| {
            let control = SettingControl::Other(Box::new(|ui, _| chord.show(ui)));
            let row = SettingRow::new(Text::message(AmmMessage::PanelToggle), control);
            rows.row(row.help(Text::message(AmmMessage::PanelToggleHelp)));
        });
    }

    // English first, then each embedded pack by its code; an unknown code selects English
    fn language(ui: &mut Ui, model: &PanelModel, out: &mut Vec<PanelIntent>) {
        let codes = PanelLocale::languages();
        let options = codes
            .iter()
            .map(|code| match code.as_str() {
                "" => DropdownOption::new(Text::message(AmmMessage::English)),
                code => DropdownOption::new(Text::user(UserText::from_model(code.to_owned()))),
            })
            .collect();
        let mut selected = Some(codes.iter().position(|code| code == model.language()).unwrap_or(0));
        let mut picked = false;
        SectionCard::new(Text::message(AmmMessage::LanguageTitle)).show(ui, |rows| {
            let control = SettingControl::Other(Box::new(|ui, _| {
                let response =
                    Dropdown::new(Id::unique(AmmId::Language), &mut selected, options).show(ui, Self::LANGUAGE_WIDTH);
                picked = response.changed();
                response
            }));
            let row = SettingRow::new(Text::message(AmmMessage::Language), control);
            rows.row(row.help(Text::message(AmmMessage::LanguageHelp)));
        });
        if let Some(code) = selected.filter(|_| picked).and_then(|index| codes.get(index)) {
            out.push(PanelIntent::Action(PanelAction::SetLanguage(code.clone())));
        }
    }
}
