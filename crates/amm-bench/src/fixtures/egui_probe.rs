#![allow(unsafe_code)]

use super::{BenchHost, RectHandler};
use amm_abi::types::AMM_ABI_MAJOR;
use amm_client::link::{ApiTable, ManagerLink};
use amm_core::structs::descriptor::Capability;
use amm_ffi::api::amm_api_for;

/// One mod connected through `amm-client` with a trivial egui handler and an Overlay lease, to count what one egui
/// pass costs in allocations.
pub struct EguiProbe {
    host: BenchHost,
    _link: ManagerLink,
}

impl EguiProbe {
    const DESCRIPTOR: &'static str = "namespace = \"test\"\nname = \"probe\"\ndisplay_name = \"probe\"\nversion = \"0.1.0\"\nauthor = \"Bench\"\n\
        config_path = \"config.hcl\"\nclaim \"Overlay\" {\n  mode = \"Shared\"\n  reason = \"bench\"\n}\n";

    pub fn new() -> Self {
        let host = BenchHost::new();
        let table_ptr = amm_api_for(AMM_ABI_MAJOR);
        // SAFETY: amm_api_for returns null or the static table of the manager
        let table = unsafe { ApiTable::new(table_ptr) }.unwrap_or_else(|error| panic!("api table: {error:?}"));
        let dir = host.game_dir().join("probe-config");
        std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("probe config dir: {error}"));
        let link = ManagerLink::new(table, Self::DESCRIPTOR, &dir, RectHandler)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        link.acquire(Capability::Overlay).unwrap_or_else(|error| panic!("acquire: {error:?}"));
        Self { host, _link: link }
    }

    pub fn host(&self) -> &BenchHost {
        &self.host
    }
}

impl Default for EguiProbe {
    fn default() -> Self {
        Self::new()
    }
}
