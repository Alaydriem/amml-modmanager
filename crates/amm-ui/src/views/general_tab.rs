use amm_core::services::settings::ColorCodec;
use amm_core::structs::descriptor::{ApplyMode, ColorEncoding, EnumOption, FieldKind, SettingField, SettingsSection};
use amm_core::structs::registry::{ModIndex, RegisteredMod};
use amml_i18n::text::{Args, Text, UserText};
use amml_ui::icon::Icon;
use amml_ui::tokens::{SpacingToken, spacing};
use amml_ui::widget::{
    Banner, BannerTone, Chip, ChipKind, ChipTone, ColourField, ColourValue, DragField, Dropdown, DropdownOption,
    SectionCard, SectionCardRows, SettingControl, SettingRow, Slider,
};
use egui::{Id, Response, Sense, Ui, Vec2};

use crate::model::{PanelAction, PanelIntent, PanelModel};
use crate::views::ValueText;
use crate::{AmmId, AmmMessage};

/// The General tab: every schema section with its non-chord fields as a section card, each field a
/// setting row with its control, its apply mode and the value a game change wrote while the
/// operator's edit was pending.
pub struct GeneralTab;

impl GeneralTab {
    /// Paper: Mod manager screen 02, a slider's width.
    const SLIDER_WIDTH: f32 = 250.0;
    /// Paper: Mod manager screen 02, the number field's width beside a slider.
    const FIELD_WIDTH: f32 = 104.0;
    /// Paper: Mod manager screen 02, a dropdown's width.
    const CHOICE_WIDTH: f32 = 250.0;

    pub fn show(ui: &mut Ui, model: &PanelModel, registered: &RegisteredMod, out: &mut Vec<PanelIntent>) {
        let sections: Vec<&SettingsSection> =
            registered.descriptor.settings.iter().filter(|section| Self::fields(section).next().is_some()).collect();
        if sections.is_empty() {
            let banner = Banner::new(BannerTone::Info, Text::message(AmmMessage::NoSettings));
            banner.body(Text::message(AmmMessage::NoSettingsBody)).show(ui);
            return;
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut position = 0;
        for (number, section) in sections.iter().enumerate() {
            if number > 0 {
                ui.add_space(<spacing::L as SpacingToken>::VALUE);
            }
            let title = Text::user(UserText::from_model(section.title.clone()));
            SectionCard::new(title).show(ui, |rows| {
                for field in Self::fields(section) {
                    let id = Id::unique(AmmId::Setting(position)).with(registered.index);
                    position += 1;
                    if let Some(value) = Self::row(rows, model, registered.index, field, id) {
                        let path = field.path.clone();
                        out.push(PanelIntent::Action(PanelAction::Edit { mod_index: registered.index, path, value }));
                    }
                }
            });
        }
    }

    fn fields(section: &SettingsSection) -> impl Iterator<Item = &SettingField> {
        section.fields.iter().filter(|field| !matches!(field.kind, FieldKind::Chord))
    }

    // one field's row; returns the new value when the operator changed it
    fn row(
        rows: &mut SectionCardRows,
        model: &PanelModel,
        index: ModIndex,
        field: &SettingField,
        id: Id,
    ) -> Option<hcl::Value> {
        let draft = model.draft(index);
        let value = model.field_value(index, field);
        let mut on = value.as_bool().unwrap_or(false);
        let mut edit = None;
        let control = match &field.kind {
            FieldKind::Bool => SettingControl::Toggle(&mut on),
            kind => SettingControl::Other(Box::new(|ui, _| Self::control(ui, id, kind, &value, &mut edit))),
        };
        let mut row = SettingRow::new(Text::user(UserText::from_model(field.label.clone())), control)
            .changed(draft.is_some_and(|draft| draft.is_edited(&field.path)));
        if !field.help.is_empty() {
            row = row.help(Text::user(UserText::from_model(field.help.clone())));
        }
        if let Some(chip) = Self::apply_chip(field.apply) {
            row = row.chip(chip);
        }
        if let Some(written) = draft.and_then(|draft| draft.game_change_note(&field.path)) {
            row = row.note(ValueText::changed_in_game(field, written));
        }
        let response = rows.row(row);
        if matches!(field.kind, FieldKind::Bool) && response.changed() {
            return Some(hcl::Value::from(on));
        }
        edit
    }

    // the control of a field that is not on/off, laid right to left; `edit` gets a changed value
    fn control(ui: &mut Ui, id: Id, kind: &FieldKind, value: &hcl::Value, edit: &mut Option<hcl::Value>) -> Response {
        match kind {
            FieldKind::Int { min, max, step } => {
                let (low, high) = (*min as f64, *max as f64);
                let mut number = value.as_i64().unwrap_or(*min) as f64;
                let response = Self::number(ui, id, (low, high, *step as f64), &mut number, |field| field.decimals(0));
                if response.changed() {
                    *edit = Some(hcl::Value::from(number.round() as i64));
                }
                response
            }
            FieldKind::Float { min, max, step, unit } => {
                let mut number = value.as_f64().unwrap_or(*min);
                let decimals = u8::try_from(ValueText::decimals(*step)).unwrap_or(u8::MAX);
                let unit = ValueText::unit(unit.as_deref());
                let response =
                    Self::number(ui, id, (*min, *max, *step), &mut number, |field| field.decimals(decimals).unit(unit));
                if response.changed() {
                    *edit = Some(hcl::Value::from(number));
                }
                response
            }
            FieldKind::Enum { options } => Self::choice(ui, id, options, value, edit),
            FieldKind::Color { alpha, encoding } => Self::colour(ui, id, (*alpha, *encoding), value, edit),
            FieldKind::Bool | FieldKind::Chord => ui.allocate_response(Vec2::ZERO, Sense::hover()),
        }
    }

    // a number field at the right with a slider before it, both on `number`
    fn number(
        ui: &mut Ui,
        id: Id,
        (min, max, step): (f64, f64, f64),
        number: &mut f64,
        look: impl FnOnce(DragField) -> DragField,
    ) -> Response {
        let reason = Text::format(
            AmmMessage::ValueRange,
            Args::new()
                .user("min", UserText::from_model(min.to_string()))
                .user("max", UserText::from_model(max.to_string())),
        );
        let field = look(DragField::new(id, number).step(step).range(min..=max, reason)).show(ui, Self::FIELD_WIDTH);
        let slider = Slider::new(number, min..=max).step(step).show(ui, Self::SLIDER_WIDTH);
        field | slider
    }

    fn choice(
        ui: &mut Ui,
        id: Id,
        options: &[EnumOption],
        value: &hcl::Value,
        edit: &mut Option<hcl::Value>,
    ) -> Response {
        let labels =
            options.iter().map(|option| DropdownOption::new(Text::user(UserText::from_model(option.label.clone()))));
        let mut selected = options.iter().position(|option| option.value == *value);
        let response = Dropdown::new(id, &mut selected, labels.collect()).show(ui, Self::CHOICE_WIDTH);
        if response.changed() {
            *edit = selected.and_then(|picked| options.get(picked)).map(|option| option.value.clone());
        }
        response
    }

    fn colour(
        ui: &mut Ui,
        id: Id,
        (alpha, encoding): (bool, ColorEncoding),
        value: &hcl::Value,
        edit: &mut Option<hcl::Value>,
    ) -> Response {
        let [r, g, b, a] = ColorCodec::to_rgba(value, encoding).unwrap_or([u8::MAX; 4]);
        let mut colour = ColourValue { rgba: [r, g, b, if alpha { a } else { u8::MAX }], alpha };
        let response = ColourField::new(id, &mut colour).show(ui);
        if response.changed() {
            let [r, g, b, a] = colour.rgba;
            *edit = Some(ColorCodec::from_rgba([r, g, b, if alpha { a } else { u8::MAX }], encoding));
        }
        response
    }

    fn apply_chip(apply: ApplyMode) -> Option<Chip> {
        let message = match apply {
            ApplyMode::Live => return None,
            ApplyMode::WorldReload => AmmMessage::AppliesAfterReload,
            ApplyMode::Restart => AmmMessage::AppliesAfterRestart,
        };
        Some(Chip::new(ChipKind::Display(ChipTone::Outline), Text::message(message)).icon(Icon::Info))
    }
}
