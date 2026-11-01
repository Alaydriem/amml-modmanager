use crate::structs::menu::CornerTarget;

/// One button of the corner row and where it sits, as `[min_x, min_y, max_x, max_y]` in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CornerSlot {
    pub target: CornerTarget,
    pub rect: [f32; 4],
}
