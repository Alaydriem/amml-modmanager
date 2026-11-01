use amml_sdk::amml_abi::AmmlLogLevel;
use amml_sdk::{Api, HookHandle, SdkError};

use crate::traits::HookHost;
use crate::types::HookSpec;

/// The AMML loader as the host the game hooks install into.
pub struct AmmlHookHost<'a> {
    api: &'a Api,
    module: &'static str,
}

impl<'a> AmmlHookHost<'a> {
    pub fn new(api: &'a Api, module: &'static str) -> Self {
        Self { api, module }
    }
}

impl HookHost for AmmlHookHost<'_> {
    fn resolve(&self, id: &str) -> Result<u64, SdkError> {
        self.api.resolve(id)
    }

    fn hook(&self, target: u64, spec: &HookSpec) -> Result<HookHandle, SdkError> {
        // SAFETY: `target` is the derived start of the game function `spec.entry` names, and `spec.detour` is the
        // amml_detour! written for that function's signature; ManagerMod::hooks pairs each entry with its detour,
        // and each original slot belongs to one hook only
        #[allow(unsafe_code)]
        unsafe {
            self.api.hook(target, spec.detour, 0, spec.original)
        }
    }

    fn unhook(&self, hook: HookHandle) -> Result<(), SdkError> {
        self.api.unhook(hook)
    }

    fn warn(&self, message: &str) {
        self.api.log(self.module, AmmlLogLevel::WARN, message);
    }
}
