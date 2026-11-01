use amm_core::structs::registry::{ModIndex, ModMask};
use amml_ui::widget::AvatarImage;
use egui::{ColorImage, Context, TextureHandle, TextureOptions, Vec2};

use crate::model::PanelModel;

/// One texture per mod slot, bound `ModMask::CAPACITY`; a slot's texture is replaced when its generation changes
/// and dropped when its icon goes.
pub struct IconTextures {
    slots: [Option<(u64, TextureHandle, AvatarImage)>; ModMask::CAPACITY],
}

impl IconTextures {
    pub fn new() -> Self {
        Self { slots: std::array::from_fn(|_| None) }
    }

    /// Brings every slot in line with the model: a slot whose generation differs loads the new
    /// texture, and a slot whose icon is gone drops its texture. A frame with no change allocates nothing.
    pub fn sync(&mut self, ctx: &Context, model: &PanelModel) {
        for (position, slot) in self.slots.iter_mut().enumerate() {
            let Ok(slot_index) = u8::try_from(position) else {
                break;
            };
            let entry = model.icon(ModIndex(slot_index));
            match (entry, slot.as_ref()) {
                (None, None) => {}
                (None, Some(_)) => *slot = None,
                (Some(entry), held) => {
                    if held.is_some_and(|(generation, _, _)| *generation == entry.generation) {
                        continue;
                    }
                    let icon = &entry.icon;
                    let size = [usize::from(icon.width), usize::from(icon.height)];
                    if size[0] * size[1] * 4 != icon.rgba.len() {
                        *slot = None;
                        continue;
                    }
                    let image = ColorImage::from_rgba_unmultiplied(size, &icon.rgba);
                    let handle = ctx.load_texture(format!("amm-icon-{slot_index}"), image, TextureOptions::LINEAR);
                    let pixels = Vec2::new(f32::from(icon.width), f32::from(icon.height));
                    let picture = AvatarImage::new(handle.id(), pixels);
                    *slot = Some((entry.generation, handle, picture));
                }
            }
        }
    }

    /// The picture for a mod's tile, or `None` when the mod has no icon. It shares the slot's brush,
    /// so asking for it allocates nothing.
    pub fn image(&self, index: ModIndex) -> Option<AvatarImage> {
        let (_, _, picture) = self.slots.get(usize::from(index.0))?.as_ref()?;
        Some(picture.clone())
    }

    /// Drops every texture, for a new egui context.
    pub fn clear(&mut self) {
        self.slots.fill_with(|| None);
    }
}

impl Default for IconTextures {
    fn default() -> Self {
        Self::new()
    }
}
