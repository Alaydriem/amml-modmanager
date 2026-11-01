use egui::Pos2;

/// What one layer's egui context saw in its last pass.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LayerSeen {
    pub pointer: Option<Pos2>,
    pub primary_down: bool,
    pub passes: u32,
}
