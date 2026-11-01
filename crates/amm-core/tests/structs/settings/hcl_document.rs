use amm_core::structs::settings::{ConfigWarning, HclDocument};

const HAND_EDITED: &str = r#"# note: keep the map small
mini_map {
  zoom = 2.0
  extra = 1
}

waypoint "home" {
  x = 10
}
"#;

#[test]
fn unparseable_text_is_reported_as_unparseable() {
    assert!(matches!(HclDocument::new("mini_map { zoom = "), Err(ConfigWarning::Unparseable)));
}

#[test]
fn values_are_read_at_nested_paths_and_labelled_blocks_are_not_paths() {
    let document = HclDocument::new(HAND_EDITED).unwrap();

    assert_eq!(document.get("mini_map.zoom"), Some(hcl::Value::from(2.0)));
    assert_eq!(document.get("mini_map.missing"), None);
    assert_eq!(document.get("waypoint.x"), None);
    assert_eq!(document.get("mini_map"), None);
    assert_eq!(document.get("bad..path"), None);
}

#[test]
fn a_negative_number_reads_as_a_number() {
    let document = HclDocument::new("offset = -5\n").unwrap();

    assert_eq!(document.get("offset"), Some(hcl::Value::from(-5)));
}

#[test]
fn comments_and_unknown_attributes_survive_a_check_and_a_write() {
    let document = HclDocument::new(HAND_EDITED).unwrap();

    let written = document.with("mini_map.zoom", &hcl::Value::from(3.5));
    let text = written.text();

    assert!(text.contains("# note: keep the map small"), "{text}");
    assert!(text.contains("extra = 1"), "{text}");
    assert!(text.contains("waypoint \"home\" {"), "{text}");
    assert!(text.contains("x = 10"), "{text}");
    assert_eq!(written.get("mini_map.zoom"), Some(hcl::Value::from(3.5)));
    assert_eq!(document.get("mini_map.zoom"), Some(hcl::Value::from(2.0)));
    assert_eq!(text, HAND_EDITED.replace("zoom = 2.0", "zoom = 3.5"));
}

#[test]
fn a_write_keeps_a_trailing_comment_on_the_changed_attribute() {
    let document = HclDocument::new("zoom = 2.0 # operator choice\nother = true\n").unwrap();

    let text = document.with("zoom", &hcl::Value::from(4)).text();

    assert_eq!(text, "zoom = 4 # operator choice\nother = true\n");
}

#[test]
fn a_write_creates_missing_unlabelled_blocks() {
    let document = HclDocument::new("# header\ntop = 1\n").unwrap();

    let written = document.with("keys.map.toggle", &hcl::Value::from("Ctrl+M"));

    assert_eq!(written.get("keys.map.toggle"), Some(hcl::Value::from("Ctrl+M")));
    assert_eq!(written.get("top"), Some(hcl::Value::from(1)));
    let reparsed = HclDocument::new(&written.text()).unwrap();
    assert_eq!(reparsed.get("keys.map.toggle"), Some(hcl::Value::from("Ctrl+M")));
    assert_eq!(written.text(), "# header\ntop = 1\nkeys {\n  map {\n    toggle = \"Ctrl+M\"\n  }\n}\n");
}

#[test]
fn a_write_into_an_empty_document_reads_back() {
    let written = HclDocument::empty().with("card.color", &hcl::Value::from(vec![255, 64, 64, 255]));

    let reparsed = HclDocument::new(&written.text()).unwrap();

    assert_eq!(reparsed.get("card.color"), Some(hcl::Value::from(vec![255, 64, 64, 255])));
}

#[test]
fn a_write_to_a_bad_path_changes_nothing() {
    let document = HclDocument::new("mini_map = 3\n").unwrap();

    assert_eq!(document.with("1bad", &hcl::Value::from(1)).text(), "mini_map = 3\n");
}

#[test]
fn a_write_through_an_attribute_replaces_it_with_a_block_in_its_place() {
    let document = HclDocument::new("first = 1\ncard = 3\nlast = 2\n").unwrap();

    let written = document.with("card.rows", &hcl::Value::from(4));

    assert_eq!(written.text(), "first = 1\ncard {\n  rows = 4\n}\nlast = 2\n");
}
