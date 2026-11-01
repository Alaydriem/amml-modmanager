use std::fs;

use amm_core::structs::event::ModEvent;
use amm_core::structs::registry::ModIndex;
use amm_ui::model::PanelAction;

use crate::support::{Descriptors, HostFixture, SimMod};

const SETTINGS: &str = r#"
section "map" {
  title = "Map"
  field "map.visible" {
    label   = "Visible"
    kind    = "bool"
    default = true
    apply   = "Live"
  }
  field "map.zoom" {
    label   = "Zoom"
    kind    = { float = { min = 0.5, max = 8.0, step = 0.25, unit = "x" } }
    default = 2.0
    apply   = "Live"
  }
}
"#;
const CONFIG: &str = "# written by hand\nunknown_attr = \"kept\"\nmap {\n  visible = true\n  zoom = 2.0\n}\n";

#[test]
fn an_edited_setting_saves_on_its_own_keeps_comments_and_unknown_keys_and_notifies_the_mod() {
    let fixture = HostFixture::new();
    let mut tuner = SimMod::new(&fixture, Descriptors::with_claims("tuner", &[], SETTINGS), CONFIG);
    tuner.drain();

    fixture.with_runtime(|runtime| {
        let edit = PanelAction::Edit {
            mod_index: ModIndex(0),
            path: "map.visible".to_owned(),
            value: hcl::Value::Bool(false),
        };
        runtime.apply_panel_actions(&[edit]);
    });
    fixture.tick(5_000);
    let changed = tuner.drain();
    let in_play = tuner.link().set_setting("map.zoom", &hcl::Value::from(3.5));
    fixture.tick(10_000);
    let path = tuner.config_path();
    drop(tuner);
    fixture.destroy();

    let text = fs::read_to_string(path).unwrap_or_else(|error| panic!("config.hcl: {error}"));
    let parsed: hcl::Value = hcl::from_str(&text).unwrap_or_else(|error| panic!("config.hcl is HCL: {error}"));
    assert!(changed.contains(&ModEvent::SettingsChanged { paths: 1 }));
    assert!(in_play.is_ok());
    assert!(text.contains("# written by hand"));
    let root = parsed.as_object().unwrap_or_else(|| panic!("config root is an object"));
    let map = root.get("map").and_then(hcl::Value::as_object).unwrap_or_else(|| panic!("map section"));
    assert_eq!(root.get("unknown_attr"), Some(&hcl::Value::from("kept")));
    assert_eq!(map.get("visible"), Some(&hcl::Value::Bool(false)));
    assert_eq!(map.get("zoom"), Some(&hcl::Value::from(3.5)));
}
