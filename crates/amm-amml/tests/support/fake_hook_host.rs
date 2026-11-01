use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::AtomicPtr;

use amml_modmanager::traits::HookHost;
use amml_modmanager::types::{HookGroup, HookSpec};
use amml_sdk::amml_abi::AmmlStatus;
use amml_sdk::{HookHandle, SdkError};

static SLOT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

/// A loader with a fixed set of entries. A hook on an entry in `refused` fails; every other hook succeeds with
/// the next handle. Records what was hooked, unhooked and warned.
pub struct FakeHookHost {
    entries: Vec<(&'static str, u64)>,
    refused: Vec<&'static str>,
    next: Mutex<u64>,
    pub hooked: Mutex<Vec<(&'static str, HookHandle)>>,
    pub unhooked: Mutex<Vec<HookHandle>>,
    pub warnings: Mutex<Vec<String>>,
}

impl FakeHookHost {
    pub fn new(entries: &[(&'static str, u64)], refused: &[&'static str]) -> Self {
        Self {
            entries: entries.to_vec(),
            refused: refused.to_vec(),
            next: Mutex::new(1),
            hooked: Mutex::new(Vec::new()),
            unhooked: Mutex::new(Vec::new()),
            warnings: Mutex::new(Vec::new()),
        }
    }

    /// A spec with no detour the fake would ever call.
    pub fn spec(entry: &'static str, group: HookGroup, required: bool) -> HookSpec {
        HookSpec { entry, detour: std::ptr::null(), original: &SLOT, group, required, feature: "the test feature" }
    }

    pub fn hooked_entries(&self) -> Vec<&'static str> {
        self.hooked.lock().unwrap().iter().map(|(entry, _)| *entry).collect()
    }
}

impl HookHost for FakeHookHost {
    fn resolve(&self, id: &str) -> Result<u64, SdkError> {
        self.entries
            .iter()
            .find(|(entry, _)| *entry == id)
            .map(|(_, value)| *value)
            .ok_or_else(|| SdkError::MissingEntry(id.to_string()))
    }

    fn hook(&self, _target: u64, spec: &HookSpec) -> Result<HookHandle, SdkError> {
        if self.refused.contains(&spec.entry) {
            return Err(SdkError::Status(AmmlStatus::FAILED));
        }
        let mut next = self.next.lock().unwrap();
        let handle = HookHandle(*next);
        *next += 1;
        self.hooked.lock().unwrap().push((spec.entry, handle));
        Ok(handle)
    }

    fn unhook(&self, hook: HookHandle) -> Result<(), SdkError> {
        self.unhooked.lock().unwrap().push(hook);
        Ok(())
    }

    fn warn(&self, message: &str) {
        self.warnings.lock().unwrap().push(message.to_string());
    }
}
