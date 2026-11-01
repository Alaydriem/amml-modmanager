use amm_core::structs::registry::{ModIndex, RegisteredMod};
use amml_i18n::text::{Args, Text, UserText};
use amml_ui::draw::{Draw, TextFit, TextStyle, TypeSize};
use amml_ui::font::FontFace;
use amml_ui::icon::Icon;
use amml_ui::text::UiText;
use amml_ui::tokens::{ColorToken, SpacingToken, color, spacing};
use amml_ui::widget::{Chip, ChipKind, ChipTone, PageHeader, PageHeaderLead, ScrollBar, TabRow, Toggle};
use egui::{Id, ScrollArea, Sense, Ui};

use crate::model::{PanelAction, PanelController, PanelIntent, PanelModel, PanelTab};
use crate::runtime::IconTextures;
use crate::views::{CapabilitiesTab, GeneralTab, KeybindsTab};
use crate::{AmmId, AmmMessage};

/// The right pane for one mod: its header, the tabs and the tab content.
pub struct ModDetailView;

impl ModDetailView {
    /// Paper: Mod manager screen 01, the space between the header and the tab row.
    const TABS_TOP: f32 = 28.0;
    /// Paper: Mod manager screen 01, the space between the tab row and the tab's content.
    const CONTENT_TOP: f32 = 24.0;
    /// Paper: Mod manager screen 01, the space between the conflict chip and the Enabled label.
    const STATE_GAP: f32 = 12.0;
    const STATE: TextStyle = TextStyle::new(TypeSize::Label, FontFace::Regular);
    const TABS: [(PanelTab, AmmMessage, Icon); 3] = [
        (PanelTab::General, AmmMessage::General, Icon::Settings),
        (PanelTab::Keybinds, AmmMessage::Keybinds, Icon::Keyboard),
        (PanelTab::Capabilities, AmmMessage::Capabilities, Icon::Lock),
    ];

    pub fn show(
        ui: &mut Ui,
        model: &PanelModel,
        controller: &PanelController,
        icons: &IconTextures,
        index: ModIndex,
        out: &mut Vec<PanelIntent>,
    ) {
        let Some(registered) = model.registered(index) else {
            return;
        };
        ui.spacing_mut().item_spacing.y = 0.0;
        Self::header(ui, model, registered, icons, out);
        ui.add_space(Self::TABS_TOP);
        let tab = controller.tab();
        Self::tabs(ui, tab, out);
        ui.add_space(Self::CONTENT_TOP);
        let area = ScrollArea::vertical().id_salt(AmmId::Pane(index, tab)).auto_shrink([false, false]);
        ScrollBar::show(ui, area, false, |ui| match tab {
            PanelTab::General => GeneralTab::show(ui, model, registered, out),
            PanelTab::Keybinds => KeybindsTab::show(ui, model, controller, registered, out),
            PanelTab::Capabilities => CapabilitiesTab::show(ui, model, registered),
        });
    }

    // the page form of the header: avatar, name, the id, version and author, the conflict chip,
    // the Enabled label and the toggle
    fn header(
        ui: &mut Ui,
        model: &PanelModel,
        registered: &RegisteredMod,
        icons: &IconTextures,
        out: &mut Vec<PanelIntent>,
    ) {
        let index = registered.index;
        let descriptor = &registered.descriptor;
        let name = UserText::from_model(descriptor.display_name.clone());
        let version = UserText::from_model(descriptor.version.clone());
        let author = UserText::from_model(descriptor.author.clone());
        let meta = vec![
            Text::user(UserText::from_model(descriptor.id())),
            Text::format(AmmMessage::Version, Args::new().user("version", version)),
            Text::format(AmmMessage::ByAuthor, Args::new().user("author", author)),
        ];
        let badge = u64::try_from(model.clash_report().badge_count(index)).unwrap_or(u64::MAX);
        PageHeader::new(PageHeaderLead::Avatar(name.clone(), icons.image(index)), Text::user(name))
            .meta(meta)
            .trailing(|ui| {
                ui.spacing_mut().item_spacing.x = <spacing::M as SpacingToken>::VALUE;
                let mut enabled = registered.enabled;
                if Toggle::new(&mut enabled).show(ui).changed() {
                    out.push(PanelIntent::Action(PanelAction::SetEnabled(index, enabled)));
                }
                Self::state(ui, registered.enabled);
                if badge > 0 {
                    ui.add_space(Self::STATE_GAP);
                    let count = Text::plural(AmmMessage::ConflictCount, badge, Args::new());
                    Chip::new(ChipKind::Display(ChipTone::Warning), count).icon(Icon::Warning).show(ui);
                }
            })
            .show(ui);
    }

    // "Enabled" in text colour, or "Disabled" dim
    fn state(ui: &mut Ui, enabled: bool) {
        let (message, colour) = if enabled {
            (AmmMessage::Enabled, <color::Text as ColorToken>::VALUE)
        } else {
            (AmmMessage::Disabled, <color::TextDim as ColorToken>::VALUE)
        };
        let words = UiText::resolve(ui.ctx(), &Text::message(message));
        let galley = Draw::galley(ui.painter(), &words, Self::STATE, colour, TextFit::Line);
        let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
        ui.painter().galley(rect.min, galley, colour);
    }

    fn tabs(ui: &mut Ui, current: PanelTab, out: &mut Vec<PanelIntent>) {
        let mut selected = Self::TABS.iter().position(|(tab, ..)| *tab == current).unwrap_or(0);
        let labels = Self::TABS.iter().map(|(_, message, _)| Text::message(*message)).collect();
        let icons = Self::TABS.iter().map(|(.., icon)| *icon).collect();
        if TabRow::new(Id::unique(AmmId::Tabs), &mut selected, labels).icons(icons).show(ui).changed() {
            out.push(PanelIntent::Action(PanelAction::SelectTab(Self::TABS[selected].0)));
        }
    }
}
