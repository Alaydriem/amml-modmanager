use std::path::PathBuf;

/// The built manager cdylib.
pub struct ManagerDll;

impl ManagerDll {
    /// Cargo builds the cdylib next to the test binary, in the `deps` folder.
    pub fn built() -> PathBuf {
        let exe = std::env::current_exe().unwrap();
        exe.parent().unwrap().join("amml_modmanager.dll")
    }
}
