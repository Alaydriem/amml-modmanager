use amm_core::config::{ExclusiveResolution, PairKey, ResolutionBook, SuspensionResolution};
use amm_core::services::clash::ClashAnalyzer;
use amm_core::services::descriptor::DescriptorChecker;
use amm_core::services::registry::ModRegistry;
use amm_core::structs::clash::{BindRef, ClashKind, ClashReport};
use amm_core::structs::descriptor::{Capability, Chord, Claim, ClaimMode, Descriptor, KeyScope, KeybindDecl};
use amm_core::structs::registry::ModIndex;

const MINIMAP: &str = include_str!("../../../fixtures/four_mods/minimap.hcl");
const FREECAM: &str = include_str!("../../../fixtures/four_mods/freecam.hcl");
const LEVELS: &str = include_str!("../../../fixtures/four_mods/levels.hcl");
const DEJAVU: &str = include_str!("../../../fixtures/four_mods/dejavu.hcl");

const KEY_A: u32 = 0x41;
const KEY_B: u32 = 0x42;

fn chord(key: u32, ctrl: bool) -> Chord {
    Chord { key, ctrl, shift: false, alt: false, f3: false }
}

fn claim(capability: Capability, mode: ClaimMode) -> Claim {
    Claim {
        capability,
        mode,
        reason: "test".into(),
        hide_vanilla_hud: false,
        block_player_turn: false,
        pass_pointer_motion: false,
    }
}

fn bind(action: &str, key: Chord, capability: Capability, scope: KeyScope) -> KeybindDecl {
    KeybindDecl {
        action: action.into(),
        label: action.into(),
        default_chord: key,
        capability,
        scope,
        config_path: format!("keys.{action}"),
    }
}

fn descriptor(id: &str, claims: Vec<Claim>, keybinds: Vec<KeybindDecl>) -> Descriptor {
    Descriptor {
        namespace: "test".into(),
        name: id.into(),
        display_name: id.into(),
        version: "0.1.0".into(),
        author: "test".into(),
        config_path: "config.hcl".into(),
        texture_budget_mb: None,
        texture_budget_count: None,
        claims,
        keybinds,
        input_passthrough: Vec::new(),
        settings: Vec::new(),
    }
}

fn default_chords(registry: &ModRegistry) -> Vec<(ModIndex, u16, Chord)> {
    registry
        .iter()
        .flat_map(|registered| {
            registered
                .descriptor
                .keybinds
                .iter()
                .enumerate()
                .map(move |(action, keybind)| (registered.index, action as u16, keybind.default_chord))
        })
        .collect()
}

fn registry_of(descriptors: Vec<Descriptor>) -> (ModRegistry, Vec<ModIndex>) {
    let mut registry = ModRegistry::new(16);
    let mut indexes = Vec::new();
    for descriptor in descriptors {
        let Ok(index) = registry.register(descriptor, true) else {
            panic!("fixture mod did not register");
        };
        indexes.push(index);
    }
    (registry, indexes)
}

fn analyze_defaults(registry: &ModRegistry, book: &ResolutionBook) -> ClashReport {
    let chords = default_chords(registry);
    let mut analyzer = ClashAnalyzer::new();
    let report = analyzer.analyze(registry, book, &chords);
    ClashReport {
        conflicts: report.conflicts.clone(),
        interactions: report.interactions.clone(),
        truncated: report.truncated,
    }
}

fn camera_pair(first: ClaimMode, second: ClaimMode) -> (ModRegistry, Vec<ModIndex>) {
    registry_of(vec![
        descriptor("alpha", vec![claim(Capability::Camera, first)], Vec::new()),
        descriptor("beta", vec![claim(Capability::Camera, second)], Vec::new()),
    ])
}

#[test]
fn two_exclusive_claims_on_camera_are_a_conflict_with_a_badge_on_both() {
    let (registry, ix) = camera_pair(ClaimMode::Exclusive, ClaimMode::Exclusive);

    let report = analyze_defaults(&registry, &ResolutionBook::new());

    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(report.conflicts[0].kind, ClashKind::Exclusive { capability: Capability::Camera, a: ix[0], b: ix[1] });
    assert!(!report.conflicts[0].reviewed);
    assert!(report.interactions.is_empty());
    assert_eq!(report.badge_count(ix[0]), 1);
    assert_eq!(report.badge_count(ix[1]), 1);
    assert_eq!(report.conflict_count(), 1);
}

#[test]
fn a_stored_resolution_marks_the_exclusive_clash_reviewed_and_clears_the_badge() {
    let (registry, ix) = camera_pair(ClaimMode::Exclusive, ClaimMode::Exclusive);
    let book = ResolutionBook::new()
        .with_exclusive(PairKey::new(Capability::Camera, "@test/beta", "@test/alpha"), ExclusiveResolution::DenySecond);

    let report = analyze_defaults(&registry, &book);

    assert_eq!(report.conflicts.len(), 1);
    assert!(report.conflicts[0].reviewed);
    assert_eq!(report.badge_count(ix[0]), 0);
    assert_eq!(report.badge_count(ix[1]), 0);
    assert_eq!(report.conflict_count(), 0);
}

#[test]
fn exclusive_and_shared_on_camera_is_an_interaction_without_a_badge() {
    let (registry, ix) = camera_pair(ClaimMode::Shared, ClaimMode::Exclusive);

    let report = analyze_defaults(&registry, &ResolutionBook::new());

    assert!(report.conflicts.is_empty());
    assert_eq!(report.interactions.len(), 1);
    assert_eq!(
        report.interactions[0].kind,
        ClashKind::Suspension { capability: Capability::Camera, exclusive: ix[1], shared: ix[0] }
    );
    assert!(!report.interactions[0].reviewed);
    assert_eq!(report.badge_count(ix[0]), 0);
    assert_eq!(report.badge_count(ix[1]), 0);
    assert_eq!(report.conflict_count(), 0);

    let book = ResolutionBook::new()
        .with_suspension(PairKey::new(Capability::Camera, "@test/alpha", "@test/beta"), SuspensionResolution::Coexist);
    assert!(analyze_defaults(&registry, &book).interactions[0].reviewed);
}

#[test]
fn two_shared_claims_on_one_capability_do_not_clash() {
    let (registry, _) = camera_pair(ClaimMode::Shared, ClaimMode::Shared);

    let report = analyze_defaults(&registry, &ResolutionBook::new());

    assert!(report.conflicts.is_empty());
    assert!(report.interactions.is_empty());
}

#[test]
fn the_four_spec_mods_give_one_interaction_and_no_conflict() {
    let checker = DescriptorChecker::new(128, 256);
    let descriptors: Vec<Descriptor> =
        [MINIMAP, FREECAM, LEVELS, DEJAVU].iter().map(|text| checker.parse(text).unwrap()).collect();
    let (registry, ix) = registry_of(descriptors);

    let report = analyze_defaults(&registry, &ResolutionBook::new());

    assert!(!report.truncated);
    assert!(report.conflicts.is_empty());
    assert_eq!(report.interactions.len(), 1);
    assert_eq!(
        report.interactions[0].kind,
        ClashKind::Suspension { capability: Capability::Camera, exclusive: ix[3], shared: ix[1] }
    );
    assert_eq!(report.conflict_count(), 0);
}

#[test]
fn two_global_binds_with_one_chord_in_two_mods_clash() {
    let (registry, ix) = registry_of(vec![
        descriptor("alpha", Vec::new(), vec![bind("go", chord(KEY_A, true), Capability::Overlay, KeyScope::Global)]),
        descriptor("beta", Vec::new(), vec![bind("run", chord(KEY_A, true), Capability::Hud, KeyScope::Global)]),
    ]);

    let report = analyze_defaults(&registry, &ResolutionBook::new());

    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(
        report.conflicts[0].kind,
        ClashKind::Key {
            chord: chord(KEY_A, true),
            a: BindRef { mod_index: ix[0], action: 0 },
            b: BindRef { mod_index: ix[1], action: 0 },
        }
    );
    assert_eq!(report.badge_count(ix[0]), 1);
    assert_eq!(report.badge_count(ix[1]), 1);
}

#[test]
fn same_mod_binds_with_one_chord_are_reported() {
    let (registry, ix) = registry_of(vec![
        descriptor(
            "alpha",
            vec![claim(Capability::Input, ClaimMode::Exclusive)],
            vec![
                bind("one", chord(KEY_A, false), Capability::Overlay, KeyScope::Global),
                bind("two", chord(KEY_A, true), Capability::Overlay, KeyScope::Global),
                bind("again", chord(KEY_A, false), Capability::Overlay, KeyScope::Global),
                bind("held", chord(KEY_B, false), Capability::Input, KeyScope::WhileHeld),
                bind("held_too", chord(KEY_B, false), Capability::Input, KeyScope::WhileHeld),
            ],
        ),
        descriptor("beta", Vec::new(), vec![bind("other", chord(KEY_B, true), Capability::Hud, KeyScope::Global)]),
    ]);

    let report = analyze_defaults(&registry, &ResolutionBook::new());

    assert_eq!(report.conflicts.len(), 2);
    assert_eq!(
        report.conflicts[0].kind,
        ClashKind::Key {
            chord: chord(KEY_A, false),
            a: BindRef { mod_index: ix[0], action: 0 },
            b: BindRef { mod_index: ix[0], action: 2 },
        }
    );
    assert_eq!(
        report.conflicts[1].kind,
        ClashKind::Key {
            chord: chord(KEY_B, false),
            a: BindRef { mod_index: ix[0], action: 3 },
            b: BindRef { mod_index: ix[0], action: 4 },
        }
    );
    assert_eq!(report.badge_count(ix[1]), 0);
}

fn global_and_held(input_mode: ClaimMode) -> ClashReport {
    let (registry, _) = registry_of(vec![
        descriptor(
            "alpha",
            Vec::new(),
            vec![bind("toggle", chord(KEY_A, false), Capability::Overlay, KeyScope::Global)],
        ),
        descriptor(
            "beta",
            vec![claim(Capability::Input, input_mode)],
            vec![bind("move", chord(KEY_A, false), Capability::Input, KeyScope::WhileHeld)],
        ),
    ]);
    analyze_defaults(&registry, &ResolutionBook::new())
}

#[test]
fn global_and_while_held_on_shared_input_clash() {
    let report = global_and_held(ClaimMode::Shared);

    assert_eq!(report.conflicts.len(), 1);
    assert!(matches!(report.conflicts[0].kind, ClashKind::Key { .. }));
}

#[test]
fn global_and_while_held_on_exclusive_input_do_not_clash() {
    let report = global_and_held(ClaimMode::Exclusive);

    assert!(report.conflicts.is_empty());
}

fn two_held(second_mode: ClaimMode, second_capability: Capability) -> ClashReport {
    let (registry, _) = registry_of(vec![
        descriptor(
            "alpha",
            vec![claim(Capability::Camera, ClaimMode::Exclusive)],
            vec![bind("hold", chord(KEY_A, false), Capability::Camera, KeyScope::WhileHeld)],
        ),
        descriptor(
            "beta",
            vec![claim(second_capability, second_mode)],
            vec![bind("hold", chord(KEY_A, false), second_capability, KeyScope::WhileHeld)],
        ),
    ]);
    analyze_defaults(&registry, &ResolutionBook::new())
}

#[test]
fn while_held_binds_on_clashing_capabilities_do_not_clash() {
    let report = two_held(ClaimMode::Shared, Capability::Camera);

    assert!(report.conflicts.iter().all(|clash| !matches!(clash.kind, ClashKind::Key { .. })));
}

#[test]
fn while_held_binds_on_compatible_capabilities_clash() {
    let report = two_held(ClaimMode::Shared, Capability::Input);

    assert_eq!(report.conflicts.len(), 1);
    assert!(matches!(report.conflicts[0].kind, ClashKind::Key { .. }));
}

#[test]
fn key_clash_is_never_reviewed() {
    let (registry, _) = registry_of(vec![
        descriptor("alpha", Vec::new(), vec![bind("go", chord(KEY_A, false), Capability::Overlay, KeyScope::Global)]),
        descriptor("beta", Vec::new(), vec![bind("go", chord(KEY_A, false), Capability::Overlay, KeyScope::Global)]),
    ]);
    let book = ResolutionBook::new()
        .with_exclusive(PairKey::new(Capability::Overlay, "@test/alpha", "@test/beta"), ExclusiveResolution::DenySecond)
        .with_suspension(PairKey::new(Capability::Overlay, "@test/alpha", "@test/beta"), SuspensionResolution::Coexist);

    let report = analyze_defaults(&registry, &book);

    assert_eq!(report.conflicts.len(), 1);
    assert!(!report.conflicts[0].reviewed);
    assert_eq!(report.conflict_count(), 1);
}

#[test]
fn disabled_mods_are_left_out() {
    let (mut registry, ix) = registry_of(vec![
        descriptor(
            "alpha",
            vec![claim(Capability::Camera, ClaimMode::Exclusive)],
            vec![bind("go", chord(KEY_A, false), Capability::Overlay, KeyScope::Global)],
        ),
        descriptor(
            "beta",
            vec![claim(Capability::Camera, ClaimMode::Exclusive)],
            vec![bind("go", chord(KEY_A, false), Capability::Overlay, KeyScope::Global)],
        ),
    ]);
    registry.set_enabled(ix[1], false);

    let report = analyze_defaults(&registry, &ResolutionBook::new());

    assert!(report.conflicts.is_empty());
    assert!(report.interactions.is_empty());
}

#[test]
fn a_second_analysis_replaces_the_first_report() {
    let (mut registry, ix) = camera_pair(ClaimMode::Exclusive, ClaimMode::Exclusive);
    let book = ResolutionBook::new();
    let mut analyzer = ClashAnalyzer::new();
    assert_eq!(analyzer.analyze(&registry, &book, &[]).conflict_count(), 1);

    registry.set_enabled(ix[0], false);

    assert_eq!(analyzer.analyze(&registry, &book, &[]).conflict_count(), 0);
}

#[test]
fn key_clashes_stop_at_the_cap_when_every_bind_shares_one_chord() {
    let descriptors: Vec<Descriptor> = (0..16)
        .map(|n| {
            let keybinds = (0..128)
                .map(|b| bind(&format!("a{b}"), chord(KEY_A, false), Capability::Overlay, KeyScope::Global))
                .collect();
            descriptor(&format!("mod{n}"), Vec::new(), keybinds)
        })
        .collect();
    let (registry, _) = registry_of(descriptors);

    let report = analyze_defaults(&registry, &ResolutionBook::new());

    assert_eq!(report.conflicts.len(), ClashAnalyzer::MAX_KEY_CLASHES);
    assert!(report.truncated);
}
