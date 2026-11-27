use std::path::Path;

use amm_core::services::PathRules;

#[test]
fn the_mods_root_is_three_folders_above_the_manager_config() {
    let config = Path::new(r"C:\game\mods\amml\@amml\modmanager\config");

    assert_eq!(PathRules::mods_root(config), Some(Path::new(r"C:\game\mods\amml")));
}

#[test]
fn a_config_folder_with_too_few_levels_has_no_mods_root() {
    assert_eq!(PathRules::mods_root(Path::new(r"modmanager\config")), None);
    assert_eq!(PathRules::mods_root(Path::new("config")), None);
}
