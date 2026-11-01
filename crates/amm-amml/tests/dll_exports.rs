use object::read::pe::PeFile64;

use super::support::manager_dll::ManagerDll;

/// The only symbols the DLL exports: `amm_get_api` for the mods it hosts and the four entry points AMML's
/// loader calls. Anything else (the `amm_host_*` calls of the host runtime, for one) would let a mod reach
/// the manager's internals.
const EXPORTS: [&str; 5] = ["amm_get_api", "amml_mod_disable", "amml_mod_enable", "amml_mod_load", "amml_mod_unload"];

#[test]
fn the_dll_exports_only_the_api_getter_and_the_amml_entry_points() {
    let bytes = std::fs::read(ManagerDll::built()).unwrap();
    let file = PeFile64::parse(&*bytes).unwrap();

    let mut names: Vec<String> = file
        .export_table()
        .unwrap()
        .expect("the DLL has an export table")
        .exports()
        .unwrap()
        .iter()
        .map(|export| String::from_utf8(export.name.expect("every export is named").to_vec()).unwrap())
        .collect();
    names.sort();

    assert_eq!(names, EXPORTS);
}
