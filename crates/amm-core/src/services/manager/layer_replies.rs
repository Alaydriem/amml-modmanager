use crate::structs::frame::{FrameReplyData, LayerKind};
use crate::structs::registry::ModIndex;
use crate::structs::snapshot::LayerEntry;

/// The last frame reply of each layer of each mod, and the mod that last reported `wants_keyboard`. Routing reads
/// it to find the layer that owns the pointer and the layer that gets key and text events. Bounds: two replies per
/// mod slot, made at construction.
pub struct LayerReplies {
    // the last reply of each mod's Hud layer (index 0) and Overlay layer (index 1)
    replies: Vec<[Option<FrameReplyData>; LayerKind::COUNT]>,
    keyboard_owner: Option<ModIndex>,
}

impl LayerReplies {
    pub fn new(slots: usize) -> Self {
        Self { replies: vec![[None; LayerKind::COUNT]; slots], keyboard_owner: None }
    }

    /// Keeps what the last frame of one layer of a mod reported. The mod keeps the keyboard until no layer of it
    /// reports `wants_keyboard`.
    pub fn record(&mut self, index: ModIndex, kind: LayerKind, reply: FrameReplyData) {
        let Some(slot) = self.replies.get_mut(usize::from(index.0)) else {
            return;
        };
        slot[kind.slot()] = Some(reply);
        let wants_keyboard = slot.iter().flatten().any(|reply| reply.wants_keyboard);
        if reply.wants_keyboard {
            self.keyboard_owner = Some(index);
        } else if !wants_keyboard && self.keyboard_owner == Some(index) {
            self.keyboard_owner = None;
        }
    }

    pub fn forget(&mut self, index: ModIndex) {
        if let Some(slot) = self.replies.get_mut(usize::from(index.0)) {
            *slot = [None; LayerKind::COUNT];
        }
        if self.keyboard_owner == Some(index) {
            self.keyboard_owner = None;
        }
    }

    /// Forgets the reply of every layer that is not in `layers`, so a layer that is drawn again after a suspension or
    /// a release starts with no pointer or keyboard wish.
    pub fn retain(&mut self, layers: &[LayerEntry]) {
        for (slot, replies) in self.replies.iter_mut().enumerate() {
            for kind in [LayerKind::Hud, LayerKind::Overlay] {
                let drawn = layers.iter().any(|entry| usize::from(entry.mod_index.0) == slot && entry.kind == kind);
                if !drawn {
                    replies[kind.slot()] = None;
                }
            }
        }
        let owner = self.keyboard_owner.and_then(|owner| self.replies.get(usize::from(owner.0)));
        if !owner.is_some_and(|replies| replies.iter().flatten().any(|reply| reply.wants_keyboard)) {
            self.keyboard_owner = None;
        }
    }

    /// The top-most of `layers` (bottom to top) under `(x, y)` whose reply wants the pointer. The Overlay layer of a
    /// mod is above its Hud layer, so it is tested first.
    pub fn pointer_layer(&self, layers: &[LayerEntry], (x, y): (f32, f32)) -> Option<LayerEntry> {
        layers.iter().rev().copied().find(|entry| {
            self.reply(entry.mod_index, entry.kind).is_some_and(|reply| {
                let [min_x, min_y, max_x, max_y] = reply.layer_rect;
                reply.wants_pointer && x >= min_x && x < max_x && y >= min_y && y < max_y
            })
        })
    }

    /// True when the last reply of a drawn Overlay layer of `layers` spans the whole `window` (width, height in
    /// physical pixels), within one pixel on each side. A small layer, such as a rail, does not.
    pub fn overlay_covers(&self, layers: &[LayerEntry], (width, height): (u32, u32)) -> bool {
        let (width, height) = (width as f32, height as f32);
        layers.iter().filter(|entry| entry.kind == LayerKind::Overlay).any(|entry| {
            self.reply(entry.mod_index, entry.kind).is_some_and(|reply| {
                let [min_x, min_y, max_x, max_y] = reply.layer_rect;
                min_x <= 1.0 && min_y <= 1.0 && max_x >= width - 1.0 && max_y >= height - 1.0
            })
        })
    }

    /// The mod that last reported `wants_keyboard`, while one of its `layers` still does.
    pub fn keyboard_owner(&self, layers: &[LayerEntry]) -> Option<ModIndex> {
        let owner = self.keyboard_owner?;
        self.keyboard_layer(layers, owner).map(|_| owner)
    }

    /// The layer of the mod in `index` that gets its key and text events: the drawn layer whose reply wants the
    /// keyboard, the Overlay layer when both do. Without one, the Overlay layer, else the Hud layer, so an `Input`
    /// holder's egui still sees its keys.
    pub fn key_layer(&self, layers: &[LayerEntry], index: ModIndex) -> Option<LayerKind> {
        self.keyboard_layer(layers, index).or_else(|| {
            let drawn = |kind: &LayerKind| layers.iter().any(|entry| entry.mod_index == index && entry.kind == *kind);
            [LayerKind::Overlay, LayerKind::Hud].into_iter().find(drawn)
        })
    }

    // The drawn layer of the mod whose reply wants the keyboard, the top-most first.
    fn keyboard_layer(&self, layers: &[LayerEntry], index: ModIndex) -> Option<LayerKind> {
        layers
            .iter()
            .rev()
            .filter(|entry| entry.mod_index == index)
            .map(|entry| entry.kind)
            .find(|kind| self.reply(index, *kind).is_some_and(|reply| reply.wants_keyboard))
    }

    fn reply(&self, index: ModIndex, kind: LayerKind) -> Option<&FrameReplyData> {
        self.replies.get(usize::from(index.0))?[kind.slot()].as_ref()
    }
}
