use std::path::{Path, PathBuf};

/// The workspace rule: a crate reaches the game and the game window only through `amml-api`. These tests scan every
/// crate's sources and manifests for the ways around it.
struct Workspace;

impl Workspace {
    const SOURCE_TOKENS: [&'static str; 10] = [
        "amml_sdk",
        "amml_msvc",
        "amml_game",
        "retour",
        "SetWindowLongPtr",
        "IDXGISwapChain",
        "ID3D12",
        "GetModuleHandle",
        "GetProcAddress",
        "amm_get_api",
    ];
    const MANIFEST_TOKENS: [&'static str; 4] = ["amml-sdk", "amml-msvc", "amml-game", "retour"];

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
    }

    fn crates() -> Vec<PathBuf> {
        let mut crates: Vec<PathBuf> = std::fs::read_dir(Self::root().join("crates"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.join("Cargo.toml").is_file())
            .collect();
        crates.sort();
        crates
    }

    fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                Self::rust_files(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }

    // `file:token` for each token that a line of `text` holds, outside a pure comment line.
    fn hits(file: &Path, text: &str, tokens: &[&str]) -> Vec<String> {
        let mut hits = Vec::new();
        for line in text.lines() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for token in tokens {
                if line.contains(token) {
                    hits.push(format!("{}: {token}", file.display()));
                }
            }
        }
        hits
    }
}

#[test]
fn no_crate_uses_a_game_escape_hatch() {
    let mut hits = Vec::new();
    for krate in Workspace::crates() {
        let mut files = Vec::new();
        Workspace::rust_files(&krate.join("src"), &mut files);
        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();
            let mut tokens = Workspace::SOURCE_TOKENS.to_vec();
            tokens.push("transmute");
            hits.extend(Workspace::hits(&file, &text, &tokens));
        }
    }

    assert!(hits.is_empty(), "{hits:#?}");
}

#[test]
fn no_crate_depends_on_the_v2_sdk() {
    let mut manifests: Vec<PathBuf> = Workspace::crates().iter().map(|krate| krate.join("Cargo.toml")).collect();
    manifests.push(Workspace::root().join("Cargo.toml"));
    let mut hits = Vec::new();
    for manifest in manifests {
        let text = std::fs::read_to_string(&manifest).unwrap();
        hits.extend(Workspace::hits(&manifest, &text, &Workspace::MANIFEST_TOKENS));
    }

    assert!(hits.is_empty(), "{hits:#?}");
}
