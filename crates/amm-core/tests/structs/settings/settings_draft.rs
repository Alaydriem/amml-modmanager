use amm_core::structs::settings::{HclDocument, SettingsDraft};

fn saved() -> HclDocument {
    match HclDocument::new("# saved by the manager\nmap {\n  zoom = 2\n  grid = false\n}\n") {
        Ok(document) => document,
        Err(warning) => panic!("fixture does not parse: {warning:?}"),
    }
}

#[test]
fn game_change_to_an_unedited_field_updates_saved_and_draft() {
    let draft = SettingsDraft::new(saved()).edit("map.zoom", hcl::Value::from(4));

    let changed = draft.apply_game_change("map.grid", hcl::Value::from(true));

    let (document, paths) = changed.commit();
    assert_eq!(document.get("map.grid"), Some(hcl::Value::from(true)));
    assert_eq!(paths, vec!["map.zoom".to_string()]);
    assert_eq!(changed.game_change_note("map.grid"), None);
    let reverted = changed.revert();
    assert!(!reverted.is_dirty());
    assert_eq!(reverted.commit().0.get("map.grid"), Some(hcl::Value::from(true)));
}

#[test]
fn game_change_to_an_edited_field_keeps_the_draft_and_records_a_note() {
    let draft = SettingsDraft::new(saved()).edit("map.zoom", hcl::Value::from(4));

    let changed = draft.apply_game_change("map.zoom", hcl::Value::from(3));

    assert_eq!(changed.game_change_note("map.zoom"), Some(&hcl::Value::from(3)));
    let (document, paths) = changed.commit();
    assert_eq!(document.get("map.zoom"), Some(hcl::Value::from(4)));
    assert_eq!(paths, vec!["map.zoom".to_string()]);
    let reverted = changed.revert();
    assert_eq!(reverted.commit().0.get("map.zoom"), Some(hcl::Value::from(3)));
    assert_eq!(reverted.game_change_note("map.zoom"), None);
}

#[test]
fn commit_returns_only_changed_paths() {
    let draft = SettingsDraft::new(saved())
        .edit("map.zoom", hcl::Value::from(2))
        .edit("map.grid", hcl::Value::from(true))
        .edit("map.grid", hcl::Value::from(false))
        .edit("map.radius", hcl::Value::from(8));

    let (document, paths) = draft.commit();

    assert_eq!(paths, vec!["map.radius".to_string()]);
    assert_eq!(document.get("map.radius"), Some(hcl::Value::from(8)));
    assert!(document.text().starts_with("# saved by the manager\n"), "{}", document.text());
    assert!(draft.is_dirty());
}

#[test]
fn an_edit_back_to_the_saved_value_is_not_dirty() {
    let draft = SettingsDraft::new(saved()).edit("map.zoom", hcl::Value::from(5));
    assert!(draft.is_dirty());

    let back = draft.edit("map.zoom", hcl::Value::from(2));

    assert!(!back.is_dirty());
    assert!(back.commit().1.is_empty());
}

#[test]
fn edits_past_the_field_limit_are_not_kept() {
    let mut draft = SettingsDraft::new(HclDocument::empty());
    for index in 0..SettingsDraft::MAX_EDITED + 1 {
        draft = draft.edit(&format!("field_{index}"), hcl::Value::from(1));
    }

    let (document, paths) = draft.commit();

    assert_eq!(paths.len(), SettingsDraft::MAX_EDITED);
    assert!(!paths.contains(&format!("field_{}", SettingsDraft::MAX_EDITED)));
    assert_eq!(document.get(&format!("field_{}", SettingsDraft::MAX_EDITED)), None);
    assert_eq!(document.get(&format!("field_{}", SettingsDraft::MAX_EDITED - 1)), Some(hcl::Value::from(1)));
}
