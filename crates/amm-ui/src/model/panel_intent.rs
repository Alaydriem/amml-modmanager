use crate::model::{PanelAction, PanelPage, RebindTarget};

/// What a view reports from one pass: an action for the host, or a change of the panel's own state
/// that the controller keeps.
#[derive(Debug, Clone, PartialEq)]
pub enum PanelIntent {
    Action(PanelAction),
    ShowPage(PanelPage),
    BeginRebind(RebindTarget),
}
