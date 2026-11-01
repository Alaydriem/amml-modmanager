use amm_core::services::settings::ChordText;
use amm_core::structs::descriptor::{Chord, KeyScope};
use amml_i18n::text::{Args, Text, UserText};
use amml_ui::AmmlUiMessage;
use amml_ui::draw::{TextStyle, TypeSize};
use amml_ui::font::FontFace;
use amml_ui::icon::Icon;
use amml_ui::tokens::{ColorToken, color};
use amml_ui::widget::{Chip, ChipKind, ChipTone, KeyChord, TableColumn, TableHeader, TableRow, TableRowKind};
use egui::{Response, Ui};

use crate::AmmMessage;
use crate::model::ModName;
use crate::model::{CapabilityText, KeybindRow, PanelModel, RebindTarget};
use crate::views::TableCell;

/// One keybind row of a keybind table: action, chord, capability and scope, in the conflict look when
/// another mod's bind has the same chord and in the capture look while it waits for its new chord.
/// The Keybinds tab and the all-keybinds page both draw their rows with it.
pub struct KeybindRowView;

impl KeybindRowView {
    /// Paper: Mod manager screens 03 and 07, the columns: action, key chord, capability and scope.
    pub const COLUMNS: [TableColumn; 4] = [
        TableColumn { width_fraction: 0.39 },
        TableColumn { width_fraction: 0.266 },
        TableColumn { width_fraction: 0.186 },
        TableColumn { width_fraction: 0.158 },
    ];
    const ACTION: TextStyle = TextStyle::new(TypeSize::Body, FontFace::Regular);
    const SCOPE: TextStyle = TextStyle::new(TypeSize::Help, FontFace::Regular);

    pub fn header() -> TableHeader {
        let heads =
            [AmmMessage::HeadAction, AmmMessage::HeadKeyChord, AmmMessage::HeadCapability, AmmMessage::HeadScope];
        TableHeader::new(
            heads.into_iter().zip(Self::COLUMNS).map(|(head, column)| (Text::message(head), column)).collect(),
        )
    }

    /// Shows the row; a click on it asks for a rebind. `capturing` is true while it waits for its new
    /// chord.
    pub fn show(ui: &mut Ui, model: &PanelModel, row: &KeybindRow, capturing: bool) -> Response {
        let kind = match row.clash_with {
            _ if capturing => TableRowKind::Capturing,
            Some(other) => TableRowKind::Conflict(Text::format(
                AmmlUiMessage::AlsoBoundBy,
                ModName::arg(model, Args::new(), "name", other),
            )),
            None => TableRowKind::Plain,
        };
        let conflict = row.clash_with.is_some();
        TableRow::new(&Self::COLUMNS, kind).show(ui, |index, ui| match index {
            0 => {
                TableCell::text(ui, &Self::action(row), Self::ACTION, <color::Text as ColorToken>::VALUE);
            }
            1 => {
                Self::chord(row.chord).conflict(conflict).show(ui);
            }
            2 => match row.capability {
                Some(capability) => {
                    let name = Text::message(CapabilityText::name(capability));
                    let chip = if row.exclusive {
                        Chip::new(ChipKind::Display(ChipTone::Accent), name).icon(Icon::Lock)
                    } else {
                        Chip::new(ChipKind::Display(ChipTone::Neutral), name)
                    };
                    chip.show(ui);
                }
                None => {
                    let none = Text::message(AmmMessage::NoCapability);
                    TableCell::text(ui, &none, Self::ACTION, <color::TextFaint as ColorToken>::VALUE);
                }
            },
            _ => {
                let scope = match row.scope {
                    KeyScope::Global => AmmMessage::ScopeGlobal,
                    KeyScope::WhileHeld => AmmMessage::ScopeWhileHeld,
                };
                TableCell::text(ui, &Text::message(scope), Self::SCOPE, <color::TextDim as ColorToken>::VALUE);
            }
        })
    }

    /// The caps of a chord, one per key.
    pub fn chord(chord: Chord) -> KeyChord {
        KeyChord::new(ChordText::keys(chord).into_iter().map(|key| Text::user(UserText::from_model(key))).collect())
    }

    /// The name of the mod a row belongs to; the panel toggle's row belongs to the Mod Manager.
    pub fn group(row: &KeybindRow) -> Text {
        match row.target {
            RebindTarget::ManagerToggle => Text::message(AmmMessage::ModManager),
            RebindTarget::Mod { .. } => Text::user(UserText::from_model(row.mod_name.to_owned())),
        }
    }

    fn action(row: &KeybindRow) -> Text {
        match row.target {
            RebindTarget::ManagerToggle => Text::message(AmmMessage::PanelToggleLabel),
            RebindTarget::Mod { .. } => Text::user(UserText::from_model(row.label.to_owned())),
        }
    }
}
