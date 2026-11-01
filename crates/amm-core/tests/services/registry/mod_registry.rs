use amm_core::config::Budgets;
use amm_core::errors::RegistryError;
use amm_core::services::registry::ModRegistry;
use amm_core::structs::descriptor::Descriptor;
use amm_core::structs::registry::ModMask;

fn descriptor(id: &str) -> Descriptor {
    Descriptor {
        namespace: "test".into(),
        name: id.to_string(),
        display_name: id.to_string(),
        version: "0.1.0".into(),
        author: "test".into(),
        config_path: "config.hcl".into(),
        texture_budget_mb: None,
        texture_budget_count: None,
        claims: Vec::new(),
        keybinds: Vec::new(),
        input_passthrough: Vec::new(),
        settings: Vec::new(),
    }
}

#[test]
fn register_past_the_default_limit_returns_too_many_mods_and_keeps_thirty_two() {
    let mut registry = ModRegistry::new(Budgets::default().max_mods);
    for n in 0..32 {
        registry.register(descriptor(&format!("mod{n}")), true).unwrap();
    }

    let result = registry.register(descriptor("extra"), true);

    assert_eq!(result, Err(RegistryError::TooManyMods));
    assert_eq!(registry.len(), 32);
    assert_eq!(registry.by_id("@test/extra"), None);
    assert!(registry.by_id("@test/mod0").is_some());
    assert!(registry.by_id("@test/mod31").is_some());
}

#[test]
fn a_limit_above_the_mask_capacity_is_cut_to_thirty_two() {
    let mut registry = ModRegistry::new(64);
    let last = (0..32).map(|n| registry.register(descriptor(&format!("mod{n}")), true).unwrap()).last().unwrap();
    let mut mask = ModMask::default();
    mask.insert(last);

    assert_eq!(registry.register(descriptor("extra"), true), Err(RegistryError::TooManyMods));
    assert!(mask.contains(last));
    assert_eq!(mask.iter().collect::<Vec<_>>(), vec![last]);
}

#[test]
fn register_with_an_existing_id_returns_duplicate_id() {
    let mut registry = ModRegistry::new(16);
    registry.register(descriptor("freecam"), true).unwrap();

    let result = registry.register(descriptor("freecam"), false);

    assert_eq!(result, Err(RegistryError::DuplicateId("@test/freecam".into())));
    assert_eq!(registry.len(), 1);
}

#[test]
fn unregister_frees_the_slot_for_a_new_mod() {
    let mut registry = ModRegistry::new(2);
    let first = registry.register(descriptor("a"), true).unwrap();
    registry.register(descriptor("b"), true).unwrap();

    let removed = registry.unregister(first).unwrap();
    let reused = registry.register(descriptor("c"), true).unwrap();

    assert_eq!(removed.descriptor.id(), "@test/a");
    assert_eq!(reused, first);
    assert_eq!(registry.by_id("@test/a"), None);
    assert_eq!(registry.get(reused).map(|m| m.descriptor.id()), Some("@test/c".to_string()));
}

#[test]
fn flags_change_only_the_named_slot() {
    let mut registry = ModRegistry::new(4);
    let a = registry.register(descriptor("a"), true).unwrap();
    let b = registry.register(descriptor("b"), true).unwrap();

    registry.set_enabled(a, false);
    registry.set_failed(b, true);

    assert_eq!(registry.get(a).map(|m| (m.enabled, m.failed)), Some((false, false)));
    assert_eq!(registry.get(b).map(|m| (m.enabled, m.failed)), Some((true, true)));
}
