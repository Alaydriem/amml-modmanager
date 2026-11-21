use object::read::pe::PeFile64;

use super::support::ManagerDll;

/// The only symbols the DLL exports: the four entry points of `amml_mod!` that AMML's loader calls. Anything else
/// (the `amm_host_*` calls of the host runtime, for one) would let a mod reach the manager's internals; mods reach
/// the manager through its registry tables only.
const EXPORTS: [&str; 4] = ["amml_mod_disable", "amml_mod_enable", "amml_mod_load", "amml_mod_unload"];

#[test]
fn the_dll_exports_only_the_mod_entry_points() {
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
