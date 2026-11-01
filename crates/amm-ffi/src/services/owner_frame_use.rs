/// What one owner used of its per-frame budgets so far in the current compose, summed over all of its layers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OwnerFrameUse {
    pub vertices: usize,
    // the vertices the owner's layers sent, drawn or not
    pub sent_vertices: usize,
    pub indices: usize,
    pub cmds: usize,
    pub deltas: usize,
    pub bytes: u64,
    pub freed: usize,
    pub frame_ns: u64,
}

impl OwnerFrameUse {
    pub fn new() -> Self {
        Self::default()
    }

    /// True when a mesh of these counts still fits the draw limits on top of what the owner drew so far.
    pub fn fits_draw(&self, (vertices, indices, cmds): (usize, usize, usize), limits: (usize, usize, usize)) -> bool {
        self.vertices.saturating_add(vertices) <= limits.0
            && self.indices.saturating_add(indices) <= limits.1
            && self.cmds.saturating_add(cmds) <= limits.2
    }

    /// Counts a drawn mesh of these counts.
    pub fn add_draw(&mut self, (vertices, indices, cmds): (usize, usize, usize)) {
        self.vertices += vertices;
        self.indices += indices;
        self.cmds += cmds;
    }
}
