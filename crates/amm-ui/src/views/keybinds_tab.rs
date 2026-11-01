use amm_core::structs::clash::{BindRef, ClashKind};
use amm_core::structs::registry::RegisteredMod;
use amml_i18n::text::{Args, Text};
use amml_ui::tokens::{SpacingToken, spacing};
use amml_ui::widget::{Banner, BannerTone};
use egui::Ui;

use crate::AmmMessage;
use crate::model::ModName;
use crate::model::{PanelController, PanelIntent, PanelModel, RebindTarget};
use crate::views::{KeybindRowView, PanelHint};

/// The Keybinds tab: action, chord, capability and scope for each keybind of one mod, under a banner
/// when its keys clash with another mod's. A click on a row waits for the next key press as the new
/// chord.
pub struct KeybindsTab;

impl KeybindsTab {
    pub fn show(
        ui: &mut Ui,
        model: &PanelModel,
        controller: &PanelController,
        registered: &RegisteredMod,
        out: &mut Vec<PanelIntent>,
    ) {
        let index = registered.index;
        ui.spacing_mut().item_spacing.y = 0.0;
        let clashes: Vec<(BindRef, BindRef)> = model
            .clash_report()
            .conflicts
            .iter()
            .filter_map(|clash| match clash.kind {
                ClashKind::Key { a, b, .. } if a.mod_index == index => Some((a, b)),
                ClashKind::Key { a, b, .. } if b.mod_index == index => Some((b, a)),
                _ => None,
            })
            .collect();
        if let Some((own, other)) = clashes.first() {
            Self::banner(ui, model, registered, clashes.len(), *own, *other);
            ui.add_space(<spacing::M as SpacingToken>::VALUE);
        }
        KeybindRowView::header().show(ui);
        let rows = model.keybind_rows();
        let own =
            rows.iter().filter(|row| matches!(row.target, RebindTarget::Mod { mod_index, .. } if mod_index == index));
        for row in own {
            let capturing = controller.rebinding() == Some(row.target);
            if KeybindRowView::show(ui, model, row, capturing).clicked() && !capturing {
                out.push(PanelIntent::BeginRebind(row.target));
            }
        }
        ui.add_space(<spacing::M as SpacingToken>::VALUE);
        PanelHint::show(ui, &Text::message(AmmMessage::KeybindsHint));
    }

    // "N key conflicts", the chord, and "is also bound by" the other mod, on one line
    fn banner(ui: &mut Ui, model: &PanelModel, registered: &RegisteredMod, count: usize, own: BindRef, other: BindRef) {
        let count = u64::try_from(count).unwrap_or(u64::MAX);
        let title = Text::plural(AmmMessage::KeyConflicts, count, Args::new());
        let tail = Text::format(AmmMessage::IsAlsoBoundBy, ModName::arg(model, Args::new(), "name", other.mod_index));
        let banner = Banner::new(BannerTone::Warning, title);
        let banner = match registered.descriptor.keybinds.get(usize::from(own.action)) {
            Some(bind) => banner.chord(KeybindRowView::chord(model.chord(own.mod_index, bind)), tail),
            None => banner.body(tail),
        };
        banner.show(ui);
    }
}
