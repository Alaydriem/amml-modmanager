use amm_core::structs::descriptor::{Capability, ClaimMode};
use amm_core::structs::registry::RegisteredMod;
use amml_i18n::text::{Args, Text, UserText};
use amml_ui::draw::{TextStyle, TypeSize};
use amml_ui::font::FontFace;
use amml_ui::icon::Icon;
use amml_ui::tokens::{ColorToken, SpacingToken, color, spacing};
use amml_ui::widget::{Chip, ChipKind, ChipTone, TableColumn, TableHeader, TableRow, TableRowKind};
use egui::Ui;

use crate::AmmMessage;
use crate::model::ModName;
use crate::model::{CapabilityText, LeaseState, PanelModel};
use crate::views::{PanelHint, TableCell};

/// The Capabilities tab: each capability with the mod's claim, its reason and the current lease state.
/// A capability the mod does not declare is a muted "Not declared" row.
pub struct CapabilitiesTab;

impl CapabilitiesTab {
    /// Paper: Mod manager screen 04, the columns: capability, claim and lease.
    const COLUMNS: [TableColumn; 3] = [
        TableColumn { width_fraction: 0.452 },
        TableColumn { width_fraction: 0.328 },
        TableColumn { width_fraction: 0.22 },
    ];
    const NAME: TextStyle = TextStyle::new(TypeSize::Body, FontFace::Semibold);
    const NOTE: TextStyle = TextStyle::new(TypeSize::Note, FontFace::Regular);

    pub fn show(ui: &mut Ui, model: &PanelModel, registered: &RegisteredMod) {
        ui.spacing_mut().item_spacing.y = 0.0;
        let name = UserText::from_model(registered.descriptor.display_name.clone());
        PanelHint::show_dim(ui, &Text::format(AmmMessage::CapabilitiesIntro, Args::new().user("mod", name)));
        ui.add_space(<spacing::M as SpacingToken>::VALUE);
        let heads = [AmmMessage::HeadCapability, AmmMessage::HeadClaim, AmmMessage::HeadLease];
        let heads = heads.into_iter().zip(Self::COLUMNS).map(|(head, column)| (Text::message(head), column));
        TableHeader::new(heads.collect()).show(ui);
        for cap in Capability::ALL {
            let claim = registered.descriptor.claims.iter().find(|claim| claim.capability == cap);
            let kind = match claim {
                Some(_) => TableRowKind::Plain,
                None => TableRowKind::Disabled(Text::message(AmmMessage::NotDeclared)),
            };
            let lease = model.lease_state(registered.index, cap);
            TableRow::new(&Self::COLUMNS, kind).show(ui, |index, ui| match (index, claim) {
                (0, _) => {
                    let text = <color::Text as ColorToken>::VALUE;
                    let dim = <color::TextDim as ColorToken>::VALUE;
                    let name = Text::message(CapabilityText::name(cap));
                    let blurb = Text::message(CapabilityText::blurb(cap));
                    match claim {
                        // the reason line goes under an undeclared name, so the blurb does not
                        None => {
                            TableCell::text(ui, &name, Self::NAME, text);
                        }
                        Some(_) => TableCell::text_over_text(ui, (&name, Self::NAME, text), (&blurb, Self::NOTE, dim)),
                    }
                }
                (1, Some(claim)) => {
                    let chip = match claim.mode {
                        ClaimMode::Exclusive => {
                            Chip::new(ChipKind::Display(ChipTone::Accent), Text::message(AmmMessage::Exclusive))
                                .icon(Icon::Lock)
                        }
                        ClaimMode::Shared => {
                            Chip::new(ChipKind::Display(ChipTone::Neutral), Text::message(AmmMessage::Shared))
                        }
                    };
                    let reason = Text::user(UserText::from_model(claim.reason.clone()));
                    TableCell::chip_over_text(ui, chip, (&reason, Self::NOTE, <color::TextDim as ColorToken>::VALUE));
                }
                (2, Some(_)) => {
                    Self::lease(model, lease).show(ui);
                }
                (2, None) => {
                    let none = Text::message(AmmMessage::NoCapability);
                    TableCell::text(ui, &none, Self::NAME, <color::TextFaint as ColorToken>::VALUE);
                }
                _ => {}
            });
        }
    }

    // the lease chip: held, suspended or blocked by another mod, or idle
    fn lease(model: &PanelModel, state: LeaseState) -> Chip {
        let chip = |tone, text| Chip::new(ChipKind::Display(tone), text);
        match state {
            LeaseState::Held => chip(ChipTone::Success, Text::message(AmmMessage::LeaseHeld)).icon(Icon::Check),
            LeaseState::Suspended { by } => {
                let text = Text::format(AmmMessage::LeaseSuspendedBy, ModName::arg(model, Args::new(), "name", by));
                chip(ChipTone::Warning, text).icon(Icon::Blocked)
            }
            LeaseState::Blocked { by } => {
                let text = Text::format(AmmMessage::LeaseBlockedBy, ModName::arg(model, Args::new(), "name", by));
                chip(ChipTone::Danger, text).icon(Icon::Blocked)
            }
            LeaseState::Idle | LeaseState::NotClaimed => chip(ChipTone::Outline, Text::message(AmmMessage::LeaseIdle)),
        }
    }
}
