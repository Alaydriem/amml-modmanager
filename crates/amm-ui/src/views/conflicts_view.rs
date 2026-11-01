use amm_core::config::{ExclusiveResolution, PairKey, SuspensionResolution};
use amm_core::services::settings::ChordText;
use amm_core::structs::clash::{BindRef, Clash, ClashKind};
use amm_core::structs::registry::ModIndex;
use amml_i18n::text::{Args, Text, UserText};
use amml_ui::draw::{Corner, Draw, TextStyle, TypeSize};
use amml_ui::font::FontFace;
use amml_ui::icon::Icon;
use amml_ui::tokens::rule::Elevation;
use amml_ui::tokens::{ColorToken, SpacingToken, color, spacing};
use amml_ui::widget::{
    Banner, BannerTone, ButtonKind, Chip, ChipKind, ChipTone, Dropdown, DropdownOption, PageHeader, PageHeaderLead,
    ScrollBar, Surface, SurfaceLevel, TextButton,
};
use egui::{Align, Id, Layout, Margin, Rect, ScrollArea, Ui, pos2, vec2};

use crate::model::{CapabilityText, ModName, PanelAction, PanelIntent, PanelModel, PanelTab};
use crate::views::{PanelHint, TableCell};
use crate::{AmmId, AmmMessage};

/// The Conflicts page: exclusive and key clashes under a badged "Conflicts" group, suspensions under
/// an unbadged "Interactions" group, one card each with its resolution.
pub struct ConflictsView;

impl ConflictsView {
    /// Paper: Mod manager screen 05, the space above each group.
    const GROUP_TOP: f32 = 28.0;
    /// Paper: Mod manager screen 05, a card's padding: 24 left, 20 right, 18 top and bottom.
    const CARD_MARGIN: Margin = Margin { left: 24, right: 20, top: 18, bottom: 18 };
    /// Paper: Mod manager screen 05, the resolution column's width and the gap before it.
    const RESOLUTION_WIDTH: f32 = 300.0;
    const COLUMN_GAP: f32 = 32.0;
    /// Paper: Mod manager screen 05, the open conflict's bar inset from the card's top and bottom.
    const BAR_INSET: f32 = 14.0;
    const CAPTION: TextStyle = TextStyle::new(TypeSize::Caption, FontFace::Semibold).tracked();
    const LABEL: TextStyle = TextStyle::new(TypeSize::Caption, FontFace::Regular);
    const TITLE: TextStyle = TextStyle::new(TypeSize::Body, FontFace::Semibold);

    pub fn show(ui: &mut Ui, model: &PanelModel, out: &mut Vec<PanelIntent>) {
        ui.spacing_mut().item_spacing.y = 0.0;
        let installed = u64::try_from(model.mod_count()).unwrap_or(u64::MAX);
        PageHeader::new(PageHeaderLead::Tile(Icon::Warning, ChipTone::Warning), Text::message(AmmMessage::Conflicts))
            .subtitle(Text::plural(AmmMessage::ConflictsSubtitle, installed, Args::new()))
            .show(ui);
        let report = model.clash_report();
        let area = ScrollArea::vertical().id_salt(AmmId::Conflicts).auto_shrink([false, false]);
        ScrollBar::show(ui, area, false, |ui| {
            ui.spacing_mut().item_spacing.y = <spacing::M as SpacingToken>::VALUE;
            ui.add_space(Self::GROUP_TOP - <spacing::M as SpacingToken>::VALUE);
            Self::group_title(ui, AmmMessage::ConflictsTitle, report.conflict_count());
            if report.conflicts.is_empty() {
                Banner::new(BannerTone::Success, Text::message(AmmMessage::NoConflicts)).show(ui);
            }
            for (position, clash) in report.conflicts.iter().enumerate() {
                Self::card(ui, model, clash, position, out);
            }
            if report.truncated {
                Banner::new(BannerTone::Info, Text::message(AmmMessage::MoreKeyClashes)).show(ui);
            }
            ui.add_space(Self::GROUP_TOP - <spacing::M as SpacingToken>::VALUE);
            Self::group_title(ui, AmmMessage::InteractionsTitle, 0);
            if report.interactions.is_empty() {
                Banner::new(BannerTone::Success, Text::message(AmmMessage::NoInteractions)).show(ui);
            }
            for (position, clash) in report.interactions.iter().enumerate() {
                Self::card(ui, model, clash, report.conflicts.len() + position, out);
            }
        });
    }

    // the group's caps title, with a warning chip of `count` when it is above 0
    fn group_title(ui: &mut Ui, title: AmmMessage, count: usize) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = <spacing::S as SpacingToken>::VALUE;
            TableCell::text(ui, &Text::message(title), Self::CAPTION, <color::TextFaint as ColorToken>::VALUE);
            if count > 0 {
                let count = Text::user(UserText::from_model(count.to_string()));
                Chip::new(ChipKind::Display(ChipTone::Warning), count).icon(Icon::Warning).show(ui);
            }
        });
    }

    fn card(ui: &mut Ui, model: &PanelModel, clash: &Clash, position: usize, out: &mut Vec<PanelIntent>) {
        let open = !matches!(clash.kind, ClashKind::Suspension { .. }) && !clash.reviewed;
        let frame = Surface::frame(SurfaceLevel::Panel, Elevation::Flat).inner_margin(Self::CARD_MARGIN);
        let rect = frame
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let left = (ui.available_width() - Self::RESOLUTION_WIDTH - Self::COLUMN_GAP).max(0.0);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.allocate_ui_with_layout(vec2(left, 0.0), Layout::top_down(Align::Min), |ui| {
                        ui.set_width(left);
                        Self::body(ui, model, clash);
                    });
                    ui.add_space(Self::COLUMN_GAP);
                    ui.allocate_ui_with_layout(vec2(Self::RESOLUTION_WIDTH, 0.0), Layout::top_down(Align::Min), |ui| {
                        ui.set_width(Self::RESOLUTION_WIDTH);
                        ui.spacing_mut().item_spacing.y = <spacing::S as SpacingToken>::VALUE;
                        let caption = Text::message(AmmMessage::ResolutionTitle);
                        TableCell::text(ui, &caption, Self::CAPTION, <color::TextFaint as ColorToken>::VALUE);
                        Self::resolution(ui, model, &clash.kind, position, out);
                    });
                });
            })
            .response
            .rect;
        if open {
            let bar = Rect::from_min_max(
                pos2(rect.left(), rect.top() + Self::BAR_INSET),
                pos2(rect.left() + <spacing::Indicator as SpacingToken>::VALUE, rect.bottom() - Self::BAR_INSET),
            );
            Draw::fill(ui.painter(), bar, Corner::Full, <color::Warning as ColorToken>::VALUE);
        }
    }

    // the kind chip, title and status chip; the detail; the two mods
    fn body(ui: &mut Ui, model: &PanelModel, clash: &Clash) {
        ui.spacing_mut().item_spacing.y = <spacing::M as SpacingToken>::VALUE;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = <spacing::M as SpacingToken>::VALUE;
            Self::kind(&clash.kind).show(ui);
            TableCell::text(ui, &Self::title(model, &clash.kind), Self::TITLE, <color::Text as ColorToken>::VALUE);
            if !matches!(clash.kind, ClashKind::Suspension { .. }) {
                let status = if clash.reviewed {
                    Chip::new(ChipKind::Display(ChipTone::Success), Text::message(AmmMessage::Resolved))
                        .icon(Icon::Check)
                } else {
                    Chip::new(ChipKind::Display(ChipTone::Warning), Text::message(AmmMessage::NeedsReview))
                        .icon(Icon::Warning)
                };
                status.show(ui);
            }
        });
        PanelHint::show_dim(ui, &Self::detail(model, &clash.kind));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = <spacing::S as SpacingToken>::VALUE;
            let label = Text::message(AmmMessage::ClashMods);
            TableCell::text(ui, &label, Self::LABEL, <color::TextFaint as ColorToken>::VALUE);
            ui.add_space(<spacing::S as SpacingToken>::VALUE);
            let (a, b) = Self::mods(&clash.kind);
            for index in [a, b] {
                Chip::new(ChipKind::Display(ChipTone::Neutral), ModName::text(model, index)).show(ui);
            }
        });
    }

    fn resolution(ui: &mut Ui, model: &PanelModel, kind: &ClashKind, position: usize, out: &mut Vec<PanelIntent>) {
        let book = &model.config().resolutions;
        let id = Id::unique(AmmId::Resolution(position));
        let prefer = |index| Text::format(AmmMessage::Prefer, ModName::arg(model, Args::new(), "name", index));
        match *kind {
            ClashKind::Exclusive { capability, a, b } => {
                let Some((a_id, b_id)) = Self::ids(model, a, b) else { return };
                let key = PairKey::new(capability, &a_id, &b_id);
                let options = vec![
                    DropdownOption::new(Text::message(AmmMessage::DenySecond)),
                    DropdownOption::new(prefer(a)),
                    DropdownOption::new(prefer(b)),
                ];
                let mut selected = Some(match book.exclusive_for(&key) {
                    Some(ExclusiveResolution::Prefer(id)) if *id == a_id => 1,
                    Some(ExclusiveResolution::Prefer(_)) => 2,
                    Some(ExclusiveResolution::DenySecond) | None => 0,
                });
                if Dropdown::new(id, &mut selected, options).show(ui, Self::RESOLUTION_WIDTH).changed() {
                    let resolution = match selected {
                        Some(1) => ExclusiveResolution::Prefer(a_id.to_string()),
                        Some(2) => ExclusiveResolution::Prefer(b_id.to_string()),
                        _ => ExclusiveResolution::DenySecond,
                    };
                    out.push(PanelIntent::Action(PanelAction::SetExclusive(key, resolution)));
                }
            }
            ClashKind::Suspension { capability, exclusive, shared } => {
                let Some((a_id, b_id)) = Self::ids(model, exclusive, shared) else { return };
                let key = PairKey::new(capability, &a_id, &b_id);
                let suspend = Text::format(AmmMessage::Suspend, ModName::arg(model, Args::new(), "name", shared));
                let options =
                    vec![DropdownOption::new(suspend), DropdownOption::new(Text::message(AmmMessage::Coexist))];
                let mut selected = Some(usize::from(book.suspension_for(&key) == Some(&SuspensionResolution::Coexist)));
                if Dropdown::new(id, &mut selected, options).show(ui, Self::RESOLUTION_WIDTH).changed() {
                    let resolution =
                        if selected == Some(1) { SuspensionResolution::Coexist } else { SuspensionResolution::Suspend };
                    out.push(PanelIntent::Action(PanelAction::SetSuspension(key, resolution)));
                }
            }
            ClashKind::Key { a, .. } => {
                PanelHint::show_dim(ui, &Text::message(AmmMessage::RebindOneKey));
                if TextButton::new(ButtonKind::Outline, Text::message(AmmMessage::OpenKeybinds)).show(ui).clicked() {
                    out.push(PanelIntent::Action(PanelAction::Select(Some(a.mod_index))));
                    out.push(PanelIntent::Action(PanelAction::SelectTab(PanelTab::Keybinds)));
                }
            }
        }
    }

    fn ids(model: &PanelModel<'_>, a: ModIndex, b: ModIndex) -> Option<(String, String)> {
        Some((model.registered(a)?.descriptor.id(), model.registered(b)?.descriptor.id()))
    }

    fn mods(kind: &ClashKind) -> (ModIndex, ModIndex) {
        match *kind {
            ClashKind::Exclusive { a, b, .. } => (a, b),
            ClashKind::Suspension { exclusive, shared, .. } => (exclusive, shared),
            ClashKind::Key { a, b, .. } => (a.mod_index, b.mod_index),
        }
    }

    fn kind(kind: &ClashKind) -> Chip {
        let (tone, message, icon) = match kind {
            ClashKind::Exclusive { .. } => (ChipTone::Warning, AmmMessage::Exclusive, Icon::Lock),
            ClashKind::Suspension { .. } => (ChipTone::Accent, AmmMessage::Suspension, Icon::Lock),
            ClashKind::Key { .. } => (ChipTone::Danger, AmmMessage::Keybind, Icon::Keyboard),
        };
        Chip::new(ChipKind::Display(tone), Text::message(message)).icon(icon)
    }

    fn title(model: &PanelModel, kind: &ClashKind) -> Text {
        let pair = |capability, first, second| {
            let args = Args::new().message("capability", CapabilityText::name(capability));
            let args = ModName::arg(model, ModName::arg(model, args, "first", first), "second", second);
            Text::format(AmmMessage::ClashPair, args)
        };
        match *kind {
            ClashKind::Exclusive { capability, a, b } => pair(capability, a, b),
            ClashKind::Suspension { capability, exclusive, shared } => pair(capability, exclusive, shared),
            ClashKind::Key { chord, .. } => {
                let chord = UserText::from_model(ChordText::format(chord));
                Text::format(AmmMessage::BoundTwice, Args::new().user("chord", chord))
            }
        }
    }

    fn detail(model: &PanelModel, kind: &ClashKind) -> Text {
        match *kind {
            ClashKind::Exclusive { capability, .. } => Text::format(
                AmmMessage::ExclusiveDetail,
                Args::new().message("capability", CapabilityText::name(capability)),
            ),
            ClashKind::Suspension { capability, exclusive, shared } => {
                let args = Args::new().message("capability", CapabilityText::name(capability));
                let args = ModName::arg(model, ModName::arg(model, args, "holder", exclusive), "other", shared);
                Text::format(AmmMessage::SuspensionDetail, args)
            }
            ClashKind::Key { a, b, .. } => {
                let args = Self::action(model, Args::new(), "first_action", a);
                let args = Self::action(model, args, "second_action", b);
                let args = ModName::arg(model, ModName::arg(model, args, "first", a.mod_index), "second", b.mod_index);
                Text::format(AmmMessage::KeyClashDetail, args)
            }
        }
    }

    // a keybind's name in placeholder `name`, or "a keybind" when the mod no longer declares it
    fn action(model: &PanelModel, args: Args, name: &'static str, bind: BindRef) -> Args {
        match model.keybind(bind.mod_index, bind.action) {
            Some(decl) => args.user(name, UserText::from_model(decl.label.clone())),
            None => args.message(name, AmmMessage::AKeybind),
        }
    }
}
