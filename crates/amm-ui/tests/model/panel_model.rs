use amm_core::config::ManagerConfig;
use amm_core::services::settings::ChordText;
use amm_core::structs::write::SaveStatus;
use amm_ui::model::{PanelModel, RebindTarget};

use crate::support::{FourMods, MINIMAP};

#[test]
fn the_keybinds_page_lists_the_panel_toggle_first_then_every_bind_of_every_mod() {
    let mods = FourMods::new(MINIMAP);
    let model = PanelModel::new(&mods.manager, "1.4.1", 0);

    let rows = model.keybind_rows();

    let declared: usize = model.mods().map(|registered| registered.descriptor.keybinds.len()).sum();
    assert_eq!(rows.len(), declared + 1);
    assert_eq!(rows[0].target, RebindTarget::ManagerToggle);
    assert_eq!((rows[0].mod_name, rows[0].label, rows[0].capability), ("", "", None));
    assert_eq!(rows[0].chord, ManagerConfig::default().toggle_chord);
    for registered in model.mods() {
        for (position, bind) in registered.descriptor.keybinds.iter().enumerate() {
            let target = RebindTarget::Mod { mod_index: registered.index, action: position as u16 };
            let found: Vec<_> = rows.iter().filter(|row| row.target == target).collect();
            assert_eq!(found.len(), 1, "{} {} is listed once", registered.descriptor.id(), bind.action);
            assert_eq!(found[0].label, bind.label);
        }
    }
}

#[test]
fn a_bind_on_the_panel_toggle_chord_is_marked_as_a_clash_on_the_toggle_row() {
    let mut mods = FourMods::new(MINIMAP);
    let toggle = ManagerConfig::default().toggle_chord;
    let registered = mods.manager.registry().get(mods.freecam).unwrap_or_else(|| panic!("freecam is registered"));
    let path = registered.descriptor.keybinds[0].config_path.clone();
    mods.manager
        .edit_draft(mods.freecam, &path, hcl::Value::from(ChordText::format(toggle)), 0)
        .unwrap_or_else(|error| panic!("{error}"));
    mods.manager.due_writes(1000).count();
    let model = PanelModel::new(&mods.manager, "1.4.1", 1000);

    let rows = model.keybind_rows();

    assert_eq!(rows[0].clash_with, Some(mods.freecam));
}

#[test]
fn the_model_reports_the_save_status_at_its_time() {
    let mut mods = FourMods::new(MINIMAP);
    mods.manager.due_writes(0).count();
    let registered = mods.manager.registry().get(mods.freecam).unwrap_or_else(|| panic!("freecam is registered"));
    let path = registered.descriptor.keybinds[0].config_path.clone();
    mods.manager.edit_draft(mods.freecam, &path, hcl::Value::from("Ctrl+K"), 5000).unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(PanelModel::new(&mods.manager, "1.4.1", 5100).save_status(), SaveStatus::Saving);
}
