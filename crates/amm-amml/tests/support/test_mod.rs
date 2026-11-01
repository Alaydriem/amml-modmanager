use std::path::Path;

use amm_abi::types::{AMM_ABI_MAJOR, AmmApi, AmmCallbacks, AmmHandle, AmmStatus};
use amm_ffi::api::amm_api_for;

/// A mod that registers with the manager of this process through the raw function table, as a client mod does.
pub struct TestMod {
    pub handle: AmmHandle,
}

impl TestMod {
    pub const DESCRIPTOR: &'static str = "namespace = \"test\"\nname = \"test-mod\"\ndisplay_name = \"Test mod\"\nversion = \"0.1.0\"\nauthor = \"A\"\nconfig_path = \"config.hcl\"\n";

    pub fn table() -> &'static AmmApi {
        // SAFETY: amm_api_for returns the manager's static table for its own major version
        unsafe { &*amm_api_for(AMM_ABI_MAJOR) }
    }

    pub fn register(config_dir: &Path) -> Self {
        let config = config_dir.to_str().unwrap();
        let callbacks = AmmCallbacks {
            size: size_of::<AmmCallbacks>() as u32,
            user: std::ptr::null_mut(),
            frame: None,
            textures_lost: None,
        };
        let mut handle = AmmHandle { slot: u16::MAX, generation: u16::MAX };
        let register = Self::table().register.unwrap();
        // SAFETY: the descriptor, the folder text, the callbacks and the handle live for the call
        let status = unsafe {
            register(
                Self::DESCRIPTOR.as_ptr(),
                Self::DESCRIPTOR.len(),
                config.as_ptr(),
                config.len(),
                &callbacks,
                &mut handle,
            )
        };
        assert_eq!(status, AmmStatus::Ok);
        Self { handle }
    }

    pub fn unregister(&self) -> AmmStatus {
        let unregister = Self::table().unregister.unwrap();
        // SAFETY: the handle came from register
        unsafe { unregister(self.handle) }
    }
}
