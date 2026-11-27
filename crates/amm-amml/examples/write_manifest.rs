//! Rewrites the `uses` block of `packaging/amml.hcl` from `ManagerMod::USES`. Run it through `mise run manifest`.

use std::path::Path;

use amml_api::AmmlMod;
use amml_modmanager::ManagerMod;

struct ManifestWriter;

impl ManifestWriter {
    fn run() -> std::io::Result<()> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("packaging").join("amml.hcl");
        let manifest = std::fs::read_to_string(&path)?;
        let written = ManagerMod::USES.write_into(&manifest);
        std::fs::write(&path, written)?;
        println!("wrote the uses block of {} for {}", path.display(), ManagerMod::id());
        Ok(())
    }
}

fn main() -> std::process::ExitCode {
    match ManifestWriter::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("write_manifest failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
