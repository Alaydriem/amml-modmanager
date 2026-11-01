use amml_ui_rules::literal::DesignLiteralScan;
use amml_ui_rules::source::SourceFiles;

#[test]
fn design_literals() {
    let files = SourceFiles::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")).exempt_dir("tokens");
    let hits = DesignLiteralScan::new(files).scan().unwrap_or_else(|error| panic!("{error}"));
    assert!(
        hits.is_empty(),
        "design literals:\n{}",
        hits.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")
    );
}
