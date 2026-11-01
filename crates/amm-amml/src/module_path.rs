use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

/// The file a loaded module was loaded from.
pub struct ModulePath;

impl ModulePath {
    const MAX_CHARS: usize = 32768;

    /// The path of `module`, or of the process executable for `None`; `None` when Windows cannot give it.
    pub fn of(module: Option<HMODULE>) -> Option<PathBuf> {
        let mut buffer = vec![0u16; Self::MAX_CHARS];
        // SAFETY: the buffer is writable for its length; a handle that names no loaded module gives 0
        #[allow(unsafe_code)]
        let length = unsafe { GetModuleFileNameW(module, &mut buffer) } as usize;
        if length == 0 || length >= buffer.len() {
            return None;
        }
        Some(PathBuf::from(OsString::from_wide(&buffer[..length])))
    }
}
