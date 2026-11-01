use amm_core::services::settings::ChordText;
use amm_core::structs::registry::RegisteredMod;
use amml_i18n::text::{Args, Text, UserText};
use amml_ui::draw::{Draw, TextFit, TextStyle, TypeSize};
use amml_ui::font::FontFace;
use amml_ui::icon::Icon;
use amml_ui::tokens::{ColorToken, SpacingToken, color, spacing};
use amml_ui::widget::{Avatar, AvatarSize, KeyCapSize, KeyChord, ListRow, NavEntry, ScrollBar, SectionHeader};
use egui::{Align2, Painter, Rect, ScrollArea, Ui, pos2};

use crate::model::{PanelAction, PanelIntent, PanelModel, PanelPage};
use crate::runtime::IconTextures;
use crate::{AmmId, AmmMessage};

/// The left column: the installed mods, then the Manager group with Manager settings, Keybinds and
/// Conflicts, and the panel toggle's chord at the bottom.
pub struct ModListView;

impl ModListView {
    /// Paper: Mod manager screen 01, the gap between two mod rows.
    const ROW_GAP: f32 = 10.0;
    /// Paper: Mod manager screen 01, the space above and below the rule over the Manager group.
    const RULE_GAP: f32 = 22.0;
    /// Paper: Mod manager screen 01, the gap between two manager entries.
    const NAV_GAP: f32 = 18.0;
    /// Paper: Mod manager screen 01, the gap between the chord and the words of the bottom hint.
    const HINT_GAP: f32 = 6.0;
    const HINT: TextStyle = TextStyle::new(TypeSize::Note, FontFace::Regular);

    pub fn show(ui: &mut Ui, model: &PanelModel, page: PanelPage, icons: &IconTextures, out: &mut Vec<PanelIntent>) {
        let area = ui.max_rect();
        ui.spacing_mut().item_spacing.y = 0.0;
        SectionHeader::new(Text::message(AmmMessage::InstalledModsTitle)).show(ui);
        let list_height = (ui.available_height() - Self::manager_group()).max(0.0);
        let list = ScrollArea::vertical().id_salt(AmmId::ModList).max_height(list_height).auto_shrink([false, true]);
        ScrollBar::show(ui, list, true, |ui| {
            ui.spacing_mut().item_spacing.y = Self::ROW_GAP;
            for registered in model.mods() {
                Self::row(ui, model, registered, page == PanelPage::Mod(registered.index), icons, out);
            }
        });
        ui.add_space(Self::RULE_GAP);
        Draw::divider(ui.painter(), area.x_range(), ui.cursor().top(), <color::Line as ColorToken>::VALUE);
        ui.add_space(Self::RULE_GAP + <spacing::Divider as SpacingToken>::VALUE);
        SectionHeader::new(Text::message(AmmMessage::ManagerTitle)).show(ui);
        let settings = NavEntry::new(Icon::Settings, Text::message(AmmMessage::ManagerSettings));
        if settings.selected(page == PanelPage::ManagerSettings).show(ui).clicked() {
            out.push(PanelIntent::ShowPage(PanelPage::ManagerSettings));
        }
        ui.add_space(Self::NAV_GAP);
        let keybinds = NavEntry::new(Icon::Keyboard, Text::message(AmmMessage::Keybinds));
        if keybinds.selected(page == PanelPage::Keybinds).show(ui).clicked() {
            out.push(PanelIntent::ShowPage(PanelPage::Keybinds));
        }
        ui.add_space(Self::NAV_GAP);
        let count = u32::try_from(model.clash_report().conflict_count()).unwrap_or(u32::MAX);
        let conflicts = NavEntry::new(Icon::Warning, Text::message(AmmMessage::Conflicts)).count(count);
        if conflicts.selected(page == PanelPage::Conflicts).show(ui).clicked() {
            out.push(PanelIntent::Action(PanelAction::Select(None)));
        }
        Self::hint(ui.painter(), area, model);
    }

    // the Manager group under the mods: the rule, its title, three entries and the hint
    fn manager_group() -> f32 {
        let rule = 2.0 * Self::RULE_GAP + <spacing::Divider as SpacingToken>::VALUE;
        let entries = 3.0 * NavEntry::ROW + 3.0 * Self::NAV_GAP;
        rule + SectionHeader::height() + entries + KeyCapSize::Small.height()
    }

    fn row(
        ui: &mut Ui,
        model: &PanelModel,
        registered: &RegisteredMod,
        selected: bool,
        icons: &IconTextures,
        out: &mut Vec<PanelIntent>,
    ) {
        let index = registered.index;
        let descriptor = &registered.descriptor;
        let name = UserText::from_model(descriptor.display_name.clone());
        let version = UserText::from_model(descriptor.version.clone());
        let badge = u32::try_from(model.clash_report().badge_count(index)).unwrap_or(u32::MAX);
        let mut enabled = registered.enabled;
        let avatar = Avatar::new(&name, AvatarSize::Regular).image(icons.image(index));
        let response = ListRow::new(avatar, Text::user(name))
            .version(Text::format(AmmMessage::Version, Args::new().user("version", version)))
            .badge(badge)
            .toggle(&mut enabled)
            .selected(selected)
            .off(!registered.enabled)
            .show(ui);
        if response.toggled {
            out.push(PanelIntent::Action(PanelAction::SetEnabled(index, enabled)));
        } else if response.row.clicked() {
            out.push(PanelIntent::Action(PanelAction::Select(Some(index))));
        }
    }

    // the panel toggle's chord, one cap per key, then "opens and closes this panel", at the bottom
    fn hint(painter: &Painter, area: Rect, model: &PanelModel) {
        let keys = ChordText::keys(model.config().toggle_chord);
        let keys = keys.into_iter().map(|key| Text::user(UserText::from_model(key))).collect();
        let size = KeyCapSize::Small;
        let centre = area.bottom() - size.height() / 2.0;
        let chord = KeyChord::new(keys).size(size).paint(painter, pos2(area.left(), centre));
        let x = chord.right() + Self::HINT_GAP;
        let faint = <color::TextFaint as ColorToken>::VALUE;
        let fit = TextFit::Truncate((area.right() - x).max(0.0));
        let words = Text::message(AmmMessage::PanelHint);
        Draw::text(painter, pos2(x, centre), Align2::LEFT_CENTER, &words, Self::HINT, faint, fit);
    }
}
