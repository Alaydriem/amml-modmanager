use std::path::Path;

use amm_ui::text::AmmPacks;

#[test]
fn every_pack_file_is_listed() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("i18n").join("packs");
    let mut on_disk: Vec<String> = std::fs::read_dir(&folder)
        .unwrap_or_else(|error| panic!("{}: {error}", folder.display()))
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension()? == "json").then(|| path.file_stem()?.to_str().map(str::to_owned))?
        })
        .collect();
    on_disk.sort();
    assert_eq!(AmmPacks::SOURCE.locales(), on_disk);
}
