use std::path::PathBuf;

use amml_modmanager::DupGuard;
use tempfile::TempDir;
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::LoadLibraryW;
use windows::core::HSTRING;

/// A copy of the built cdylib under the name AMML loads it by, in a folder of its own, loaded into this process
/// by its full path. Freed on drop.
pub struct ManagerDll {
    pub module: HMODULE,
    pub path: PathBuf,
    _dir: TempDir,
}

impl ManagerDll {
    /// Cargo builds the cdylib next to the test binary, in the `deps` folder.
    pub fn built() -> PathBuf {
        let exe = std::env::current_exe().unwrap();
        exe.parent().unwrap().join("amml_modmanager.dll")
    }

    pub fn load_copy() -> Self {
        Self::load_copy_as(DupGuard::MODULE_NAME)
    }

    /// A copy under another file name, such as the C++ manager's.
    pub fn load_copy_as(file_name: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(file_name);
        std::fs::copy(Self::built(), &path).unwrap();
        // SAFETY: the path names a copy of this crate's cdylib, whose load runs no code of ours
        let module = unsafe { LoadLibraryW(&HSTRING::from(path.as_os_str())) }.unwrap();
        Self { module, path, _dir: dir }
    }
}

impl Drop for ManagerDll {
    fn drop(&mut self) {
        // SAFETY: the module was loaded by `load_copy` and nothing of it is used after the test
        let _ = unsafe { FreeLibrary(self.module) };
    }
}
