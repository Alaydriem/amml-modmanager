/// Where the open panel's controls are, as `[min_x, min_y, max_x, max_y]` in back-buffer pixels: the close button,
/// and the search field when the panel has one (`None`: the panel draws no search field).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelLayout {
    pub close: [f32; 4],
    pub search: Option<[f32; 4]>,
}
