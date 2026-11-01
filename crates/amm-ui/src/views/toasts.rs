use amm_core::structs::toast::{Toast, ToastKind};
use amml_ui::widget::{Toast as ToastCard, ToastTone};
use egui::{Context, Id, LayerId, Order, Rect, Ui, UiBuilder, pos2, vec2};

use crate::AmmId;
use crate::model::{PanelModel, ToastText};

/// The toast stack in the bottom-right corner, newest at the bottom, inside a 24 px screen margin.
/// While the menu button shows, the stack sits above it, so no toast covers the button. While the
/// panel is open and the stack does not fit below it, the stack moves inside the panel's
/// bottom-right corner, so no toast ever crosses the panel's bottom edge.
pub struct ToastsView;

impl ToastsView {
    pub const MARGIN: f32 = 24.0;
    /// Paper: Mod manager screen 06, a toast's width.
    const WIDTH: f32 = 480.0;
    /// Paper: Mod manager screen 06, the gap between two toasts.
    const GAP: f32 = 8.0;

    pub fn show(ctx: &Context, screen: Rect, panel: Option<Rect>, corner: Option<Rect>, model: &PanelModel) {
        let toasts: Vec<&Toast> = model.toasts().collect();
        if toasts.is_empty() {
            return;
        }
        let layer = LayerId::new(Order::Middle, Id::unique(AmmId::Toasts));
        let mut ui = Ui::new(ctx.clone(), Id::unique(AmmId::Toasts), UiBuilder::new().layer_id(layer).max_rect(screen));
        // each card's height for its wrapped text, from a pass that paints nothing
        let heights: Vec<f32> = toasts.iter().map(|toast| Self::measure(&mut ui, screen, toast, model)).collect();
        let stack = heights.iter().sum::<f32>() + Self::GAP * (heights.len() - 1) as f32;
        let (right, bottom) = Self::anchor(screen, panel, corner, stack);
        let top_limit = screen.top() + Self::MARGIN;
        let mut y = bottom;
        for (toast, height) in toasts.iter().zip(heights).rev() {
            if y - height < top_limit {
                break;
            }
            let rect = Rect::from_min_max(pos2(right - Self::WIDTH, y - height), pos2(right, y));
            Self::card(toast, model).show(&mut ui.new_child(UiBuilder::new().max_rect(rect)));
            y -= height + Self::GAP;
        }
    }

    /// The bottom-right corner of a stack `stack` points high, given the open panel's rect and the
    /// menu button's rect while it shows.
    pub fn anchor(screen: Rect, panel: Option<Rect>, menu_button: Option<Rect>, stack: f32) -> (f32, f32) {
        let right = screen.right() - Self::MARGIN;
        let corner = screen.bottom() - Self::MARGIN;
        let bottom = menu_button.map_or(corner, |button| corner.min(button.top() - Self::GAP));
        let Some(panel) = panel else {
            return (right, bottom);
        };
        if bottom - stack >= panel.bottom() + Self::MARGIN {
            return (right, bottom);
        }
        (right.min(panel.right() - Self::MARGIN), bottom.min(panel.bottom() - Self::MARGIN))
    }

    fn measure(ui: &mut Ui, screen: Rect, toast: &Toast, model: &PanelModel) -> f32 {
        let place = Rect::from_min_size(screen.min, vec2(Self::WIDTH, screen.height()));
        let mut probe = ui.new_child(UiBuilder::new().max_rect(place).invisible().sizing_pass());
        Self::card(toast, model).show(&mut probe);
        probe.min_rect().height()
    }

    // the card: the title, message and hint from `ToastText`, the age and the time left
    fn card(toast: &Toast, model: &PanelModel) -> ToastCard {
        let words = ToastText::render(&toast.kind, model);
        let age = model.now_ms().saturating_sub(toast.created_ms);
        let remaining = 1.0 - age as f32 / PanelModel::TOAST_TTL_MS as f32;
        let mut card = ToastCard::new(Self::tone(&toast.kind), words.title).age_ms(age).remaining(remaining);
        if let Some(message) = words.message {
            card = card.message(message);
        }
        if let Some(hint) = words.hint {
            card = card.hint(hint);
        }
        card
    }

    fn tone(kind: &ToastKind) -> ToastTone {
        match kind {
            ToastKind::Denied { .. } | ToastKind::Blocked { .. } => ToastTone::Danger,
            ToastKind::FrameDisabled(_) | ToastKind::SaveFailed => ToastTone::Warning,
        }
    }
}
