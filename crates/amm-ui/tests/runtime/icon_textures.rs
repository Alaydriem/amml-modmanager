use amm_ui::model::PanelModel;
use amm_ui::runtime::IconTextures;
use egui::Context;

use crate::support::{FourMods, IconFixture, MINIMAP};

struct TextureCount;

impl TextureCount {
    fn allocated(ctx: &Context) -> usize {
        ctx.tex_manager().read().num_allocated()
    }
}

#[test]
fn sync_uploads_once_per_generation() {
    let mut mods = FourMods::new(MINIMAP);
    let ctx = Context::default();
    let mut icons = IconTextures::new();
    let base = TextureCount::allocated(&ctx);
    mods.manager
        .set_icon(mods.freecam, Some(IconFixture::solid(8, 8, [255, 0, 0, 255])))
        .expect("freecam is registered");

    icons.sync(&ctx, &PanelModel::new(&mods.manager, "1.4.1", 0));
    icons.sync(&ctx, &PanelModel::new(&mods.manager, "1.4.1", 0));

    assert_eq!(TextureCount::allocated(&ctx), base + 1);
    assert!(icons.image(mods.freecam).is_some());
    assert!(icons.image(mods.minimap).is_none());
}

#[test]
fn a_new_generation_replaces_the_texture() {
    let mut mods = FourMods::new(MINIMAP);
    let ctx = Context::default();
    let mut icons = IconTextures::new();
    mods.manager
        .set_icon(mods.freecam, Some(IconFixture::solid(8, 8, [255, 0, 0, 255])))
        .expect("freecam is registered");
    icons.sync(&ctx, &PanelModel::new(&mods.manager, "1.4.1", 0));
    let first = icons.image(mods.freecam).expect("the icon is uploaded");

    mods.manager
        .set_icon(mods.freecam, Some(IconFixture::solid(16, 8, [0, 255, 0, 255])))
        .expect("freecam is registered");
    icons.sync(&ctx, &PanelModel::new(&mods.manager, "1.4.1", 0));
    let second = icons.image(mods.freecam).expect("the new icon is uploaded");

    assert_ne!(first.texture, second.texture);
    assert_eq!((second.size.x, second.size.y), (16.0, 8.0));
}

#[test]
fn cleared_icon_drops_its_texture() {
    let mut mods = FourMods::new(MINIMAP);
    let ctx = Context::default();
    let mut icons = IconTextures::new();
    mods.manager
        .set_icon(mods.freecam, Some(IconFixture::solid(8, 8, [255, 0, 0, 255])))
        .expect("freecam is registered");
    icons.sync(&ctx, &PanelModel::new(&mods.manager, "1.4.1", 0));
    assert!(icons.image(mods.freecam).is_some());

    mods.manager.set_icon(mods.freecam, None).expect("freecam is registered");
    icons.sync(&ctx, &PanelModel::new(&mods.manager, "1.4.1", 0));

    assert_eq!(icons.image(mods.freecam), None);
}

#[test]
fn clear_then_sync_uploads_again() {
    let mut mods = FourMods::new(MINIMAP);
    let mut icons = IconTextures::new();
    mods.manager
        .set_icon(mods.freecam, Some(IconFixture::solid(8, 8, [255, 0, 0, 255])))
        .expect("freecam is registered");
    let old = Context::default();
    icons.sync(&old, &PanelModel::new(&mods.manager, "1.4.1", 0));

    icons.clear();
    assert_eq!(icons.image(mods.freecam), None);
    let fresh = Context::default();
    let base = TextureCount::allocated(&fresh);
    icons.sync(&fresh, &PanelModel::new(&mods.manager, "1.4.1", 0));

    assert_eq!(TextureCount::allocated(&fresh), base + 1);
    assert!(icons.image(mods.freecam).is_some());
}
