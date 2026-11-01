use amml_modmanager::types::GameSlot;
use amml_modmanager::{DupGuard, ManagerMod};
use amml_sdk::AmmlMod;
use hcl::Expression;

const MANIFEST: &str = include_str!("../packaging/amml.hcl");

fn attribute(key: &str) -> Expression {
    let body = hcl::parse(MANIFEST).unwrap();
    body.attributes().find(|attribute| attribute.key() == key).unwrap().expr().clone()
}

#[test]
fn the_manifest_names_the_mod_and_the_dll_mods_look_for() {
    assert_eq!(attribute("namespace"), Expression::String(ManagerMod::NAMESPACE.to_string()));
    assert_eq!(attribute("name"), Expression::String(ManagerMod::NAME.to_string()));
    assert_eq!(ManagerMod::id(), ManagerMod::MODULE);
    assert_eq!(attribute("entry"), Expression::String(DupGuard::MODULE_NAME.to_string()));
}

#[test]
fn the_manifest_lists_every_hooked_and_called_entry() {
    let Expression::Array(entries) = attribute("entries") else { panic!("entries is not an array") };
    for spec in ManagerMod::hooks() {
        assert!(entries.contains(&Expression::String(spec.entry.to_string())), "{} is not listed", spec.entry);
    }
    for slot in GameSlot::ALL {
        assert!(entries.contains(&Expression::String(slot.entry().to_string())), "{} is not listed", slot.entry());
    }
}
