/// Where a mod draws the live game picture: `[min_x, min_y, max_x, max_y]` in back-buffer pixels. A pointer inside
/// it stands for the matching place of the full game window, so a game screen shown in the picture can be used.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameViewport {
    rect: [f32; 4],
}

impl GameViewport {
    /// `None` for a rect with a NaN or no area.
    pub fn new(rect: [f32; 4]) -> Option<Self> {
        let [min_x, min_y, max_x, max_y] = rect;
        let valid = rect.iter().all(|value| value.is_finite()) && max_x > min_x && max_y > min_y;
        valid.then_some(Self { rect })
    }

    pub fn rect(&self) -> [f32; 4] {
        self.rect
    }

    /// True when `point` is inside; the max edges are outside.
    pub fn contains(&self, (x, y): (f32, f32)) -> bool {
        let [min_x, min_y, max_x, max_y] = self.rect;
        x >= min_x && x < max_x && y >= min_y && y < max_y
    }

    /// The point of a full window of `size` that `point` stands for, or `None` when `point` is outside.
    pub fn map(&self, point: (f32, f32), (width, height): (f32, f32)) -> Option<(f32, f32)> {
        if !self.contains(point) {
            return None;
        }
        let [min_x, min_y, max_x, max_y] = self.rect;
        let x = (point.0 - min_x) * width / (max_x - min_x);
        let y = (point.1 - min_y) * height / (max_y - min_y);
        Some((x, y))
    }
}
